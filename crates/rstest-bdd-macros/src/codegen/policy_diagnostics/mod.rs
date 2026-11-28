//! Harness and attribute-policy diagnostics, decided once per expansion.
//!
//! Procedural macros cannot evaluate trait methods, so the macros recognize
//! first-party adapters by the spelling of the supplied paths. When a path is
//! not the spelling they expect, the generated test quietly changes shape:
//! `#[gpui::test]` disappears, or the build fails with an error that names
//! neither the cause nor the fix.
//!
//! This module owns that diagnosis end to end. A boundary supplies the paths
//! it was given, [`classify`](classify) turns each into a [`SlotClass`],
//! [`diagnose`](model::diagnose) decides the findings, and the renderer turns
//! them into warnings carrying a stable code and a fix. Resolution is pure and
//! side-effect free: only the owning boundary calls
//! [`SharedPolicyDiagnostics::emit_diagnostics`], and `scenarios!` calls it
//! once however many tests it generates, so the cardinality contract is
//! structural rather than counted.
//!
//! The module is deliberately separate from `codegen::scenario::emit`, which
//! assembles the generated test item; this one owns only diagnostics.

mod classify;
mod emit;
mod inputs;
mod message;
mod model;
mod render;
mod resolution;

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;

use proc_macro2::{Ident, TokenStream as TokenStream2};

#[cfg(test)]
pub(crate) use self::classify::ResolveError;
#[cfg(test)]
pub(crate) use self::model::Framework;
use self::model::{Framework as ProbeFramework, PolicyInputs, diagnose};
pub(crate) use self::{
    classify::{CrateResolver, classify_harness_path, classify_policy_path, default_resolver},
    model::{PolicyFindings, SlotClass},
    resolution::selected_hint,
};
use crate::codegen::{HarnessApiResolution, resolve_harness_api_with};

/// The diagnostics one expansion boundary owns.
///
/// The two API resolutions are what generated code consumes; the state is what
/// diagnostics consume. They are separate because `scenarios!` classifies the
/// *explicit* `harness` argument but generates against the path the runtime
/// alias supplies.
#[derive(Clone)]
pub(crate) struct SharedPolicyDiagnostics {
    /// API resolution for the harness path generated against, if any.
    pub(crate) harness: Option<HarnessApiResolution>,
    /// API resolution for the supplied attribute-policy path, if any.
    pub(crate) attributes: Option<HarnessApiResolution>,
    /// Classified slots, spans, and findings; absent for an API-only value.
    state: Option<DiagnosticState>,
}

/// The classified inputs and decided findings for one boundary.
#[derive(Clone)]
struct DiagnosticState {
    /// The explicit user arguments, already classified.
    inputs: PolicyInputs,
    /// A Cargo state that could not be read, when one was encountered.
    manifest_error: Option<String>,
    /// The terminal identifier of each explicit path.
    slots: SlotIdents,
    /// What the classifier found.
    findings: PolicyFindings,
    /// The resolver suggested paths are rendered against.
    resolver: &'static dyn CrateResolver,
}

/// The terminal identifiers a rendered diagnostic can point at.
#[derive(Clone)]
struct SlotIdents {
    /// Terminal identifier of the explicit `harness` path.
    harness: Option<Ident>,
    /// Terminal identifier of the explicit `attributes` path.
    attributes: Option<Ident>,
}

/// The paths one `#[scenario]` expansion supplies.
#[derive(Clone, Copy)]
pub(crate) struct ScenarioRequest<'a> {
    /// The explicit `harness = ...` argument.
    pub(crate) harness: Option<&'a syn::Path>,
    /// The explicit `attributes = ...` argument.
    pub(crate) attributes: Option<&'a syn::Path>,
    /// The facts about the annotated function.
    pub(crate) facts: ScenarioFacts,
}

/// The paths one `scenarios!` expansion supplies.
#[derive(Clone, Copy)]
pub(crate) struct ScenariosRequest<'a> {
    /// The explicit `harness = ...` argument.
    pub(crate) harness: Option<&'a syn::Path>,
    /// The path generated code runs against, after any alias.
    pub(crate) effective_harness: Option<&'a syn::Path>,
    /// The explicit `attributes = ...` argument.
    pub(crate) attributes: Option<&'a syn::Path>,
    /// Whether the deprecated `runtime` alias was supplied.
    pub(crate) runtime_alias: bool,
}

/// Either shape one boundary diagnoses.
#[derive(Clone, Copy)]
enum BoundaryRequest<'a> {
    /// A `#[scenario]` expansion.
    Scenario(ScenarioRequest<'a>),
    /// A `scenarios!` expansion.
    Scenarios(ScenariosRequest<'a>),
}

impl<'a> BoundaryRequest<'a> {
    /// The explicit harness argument, if any.
    fn explicit_harness(self) -> Option<&'a syn::Path> {
        match self {
            Self::Scenario(request) => request.harness,
            Self::Scenarios(request) => request.harness,
        }
    }

    /// The path generated code runs against, if any.
    fn effective_harness(self) -> Option<&'a syn::Path> {
        match self {
            Self::Scenario(request) => request.harness,
            Self::Scenarios(request) => request.effective_harness,
        }
    }

    /// The explicit attributes argument, if any.
    fn attributes(self) -> Option<&'a syn::Path> {
        match self {
            Self::Scenario(request) => request.attributes,
            Self::Scenarios(request) => request.attributes,
        }
    }

    /// The classified inputs, with the supplied classes.
    fn inputs(self, harness: SlotClass, attributes: SlotClass) -> PolicyInputs {
        match self {
            Self::Scenario(request) => PolicyInputs::Scenario {
                harness,
                attributes,
                is_async: request.facts.is_async,
                has_executor_attr: request.facts.has_executor_attr,
                has_gpui_test_attr: request.facts.has_gpui_test_attr,
            },
            Self::Scenarios(request) => PolicyInputs::Scenarios {
                harness,
                attributes,
                runtime_alias: request.runtime_alias,
            },
        }
    }
}

/// The facts the `#[scenario]` boundary knows about its function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScenarioFacts {
    /// Whether the annotated function is `async`.
    pub(crate) is_async: bool,
    /// Whether the function already carries an executor attribute.
    pub(crate) has_executor_attr: bool,
    /// Whether the function already carries `#[gpui::test]`.
    pub(crate) has_gpui_test_attr: bool,
}

impl SharedPolicyDiagnostics {
    /// Resolve the inputs of a `#[scenario]` expansion.
    pub(crate) fn for_scenario(request: ScenarioRequest<'_>) -> Self {
        Self::for_scenario_with(request, default_resolver())
    }

    /// Resolve a `#[scenario]` expansion against an injected resolver.
    ///
    /// Boundary verification uses this to exercise the production wiring with a
    /// chosen Cargo answer instead of the unit-test stub.
    pub(crate) fn for_scenario_with(
        request: ScenarioRequest<'_>,
        resolver: &'static dyn CrateResolver,
    ) -> Self {
        Self::resolve(BoundaryRequest::Scenario(request), resolver)
    }

    /// Resolve the inputs of a `scenarios!` expansion.
    ///
    /// `effective_harness` is the path the generated tests run against, which
    /// the deprecated `runtime` alias supplies when the user wrote no harness.
    /// Classification deliberately uses `harness` instead (decision D21): an
    /// alias is not a supplied harness.
    pub(crate) fn for_scenarios(request: ScenariosRequest<'_>) -> Self {
        Self::for_scenarios_with(request, default_resolver())
    }

    /// Resolve a `scenarios!` expansion against an injected resolver.
    pub(crate) fn for_scenarios_with(
        request: ScenariosRequest<'_>,
        resolver: &'static dyn CrateResolver,
    ) -> Self {
        Self::resolve(BoundaryRequest::Scenarios(request), resolver)
    }

    /// Resolve API paths only, for direct code generation.
    ///
    /// Direct generation never emits, so this value carries no findings. It
    /// exists so `codegen::scenario` can resolve the paths a caller supplied
    /// without pretending a boundary ran.
    pub(crate) fn resolve_api(harness: Option<&syn::Path>, attributes: Option<&syn::Path>) -> Self {
        Self::resolve_api_with(harness, attributes, default_resolver())
    }

    /// Resolve API paths only, against an injected resolver.
    pub(crate) fn resolve_api_with(
        harness: Option<&syn::Path>,
        attributes: Option<&syn::Path>,
        resolver: &'static dyn CrateResolver,
    ) -> Self {
        Self {
            harness: harness.map(|path| resolve_harness_api_with(path, resolver)),
            attributes: attributes.map(|path| resolve_harness_api_with(path, resolver)),
            state: None,
        }
    }

    /// Classify the explicit paths and decide the findings.
    fn resolve(request: BoundaryRequest<'_>, resolver: &'static dyn CrateResolver) -> Self {
        let manifest_error = first_manifest_error(resolver);
        let explicit_harness = request.explicit_harness();
        let attributes = request.attributes();
        let harness = explicit_harness.map_or(SlotClass::Absent, |path| {
            classify_harness_path(path, resolver)
        });
        let attribute_slot = attributes.map_or(SlotClass::Absent, |path| {
            classify_policy_path(path, resolver)
        });
        let inputs = request.inputs(harness, attribute_slot);
        Self {
            harness: request
                .effective_harness()
                .map(|path| resolve_harness_api_with(path, resolver)),
            attributes: attributes.map(|path| resolve_harness_api_with(path, resolver)),
            state: Some(DiagnosticState {
                inputs,
                manifest_error,
                slots: SlotIdents {
                    harness: explicit_harness.and_then(terminal_ident),
                    attributes: attributes.and_then(terminal_ident),
                },
                findings: diagnose(inputs),
                resolver,
            }),
        }
    }

    /// The findings this boundary decided.
    ///
    /// An API-only value reports no findings, which is truthful: no boundary
    /// diagnosed it. Only verification reads this: emission renders the same
    /// state directly.
    #[cfg(test)]
    pub(crate) fn findings(&self) -> PolicyFindings {
        self.state
            .as_ref()
            .map_or_else(PolicyFindings::default, |state| state.findings)
    }

    /// The errors this boundary would report, without emitting anything.
    ///
    /// `scenarios!` needs this separately: an invocation that generates no test
    /// cannot fail per scenario, and compiled before this diagnostic existed,
    /// so the boundary decides whether the error applies after generation.
    pub(crate) fn errors(&self) -> Option<TokenStream2> {
        let state = self.state.as_ref()?;
        if let Some(error) = &state.manifest_error {
            let message = format!(
                "rstest-bdd cannot read the Cargo manifest, so harness and \
                 attribute-policy paths cannot be resolved: {error}"
            );
            return Some(
                syn::Error::new(proc_macro2::Span::call_site(), message).into_compile_error(),
            );
        }
        render::errors(state)
    }

    /// The warning tokens this boundary owns. Pure.
    pub(crate) fn warning_tokens(&self) -> TokenStream2 {
        self.state.as_ref().map_or_else(TokenStream2::new, |state| {
            emit::warning_tokens(&render::warnings(state))
        })
    }

    /// Emit the native warnings this boundary owns.
    ///
    /// This is the command half: call it at most once per boundary, and only
    /// when [`Self::errors`] is `None`, because nightly emission is a side
    /// effect.
    pub(crate) fn emit_native_warnings(&self) {
        if let Some(state) = self.state.as_ref() {
            emit::emit_native(&render::warnings(state));
        }
    }
}

/// The first Cargo state that could not be read, if any.
///
/// The adapter crates are probed once per boundary so an unreadable manifest is
/// reported as a hard error rather than silently disabling recognition.
fn first_manifest_error(resolver: &dyn CrateResolver) -> Option<String> {
    [ProbeFramework::Tokio, ProbeFramework::Gpui]
        .into_iter()
        .find_map(|framework| {
            let spec = classify::adapter_spec(framework);
            resolver
                .resolved_root(spec.package_name)
                .err()
                .map(|error| {
                    format!(
                        "rstest-bdd cannot read the Cargo manifest, so harness and \
                     attribute-policy paths cannot be resolved: {error}"
                    )
                })
        })
}

/// The terminal identifier of a supplied path.
fn terminal_ident(path: &syn::Path) -> Option<Ident> {
    path.segments.last().map(|segment| segment.ident.clone())
}
