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
pub(crate) use self::model::Framework;
use self::model::{PolicyInputs, diagnose};
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

/// The shape one boundary diagnoses.
#[derive(Clone, Copy)]
enum Shape {
    /// A `#[scenario]` expansion.
    Scenario {
        /// Whether the annotated function is `async`.
        is_async: bool,
        /// Whether the function already carries an executor attribute.
        has_executor_attr: bool,
        /// Whether the function already carries `#[gpui::test]`.
        has_gpui_test_attr: bool,
    },
    /// A `scenarios!` expansion.
    Scenarios {
        /// Whether the deprecated `runtime` alias was supplied.
        runtime_alias: bool,
    },
}

impl SharedPolicyDiagnostics {
    /// Resolve the inputs of a `#[scenario]` expansion.
    pub(crate) fn for_scenario(
        harness: Option<&syn::Path>,
        attributes: Option<&syn::Path>,
        facts: ScenarioFacts,
    ) -> Self {
        Self::resolve(
            harness,
            harness,
            attributes,
            Shape::Scenario {
                is_async: facts.is_async,
                has_executor_attr: facts.has_executor_attr,
                has_gpui_test_attr: facts.has_gpui_test_attr,
            },
        )
    }

    /// Resolve the inputs of a `scenarios!` expansion.
    ///
    /// `effective_harness` is the path the generated tests run against, which
    /// the deprecated `runtime` alias supplies when the user wrote no harness.
    /// Classification deliberately uses `harness` instead (decision D21): an
    /// alias is not a supplied harness.
    pub(crate) fn for_scenarios(
        harness: Option<&syn::Path>,
        effective_harness: Option<&syn::Path>,
        attributes: Option<&syn::Path>,
        runtime_alias: bool,
    ) -> Self {
        Self::resolve(
            harness,
            effective_harness,
            attributes,
            Shape::Scenarios { runtime_alias },
        )
    }

    /// Resolve API paths only, for direct code generation.
    ///
    /// Direct generation never emits, so this value carries no findings. It
    /// exists so `codegen::scenario` can resolve the paths a caller supplied
    /// without pretending a boundary ran.
    pub(crate) fn resolve_api(harness: Option<&syn::Path>, attributes: Option<&syn::Path>) -> Self {
        let resolver = default_resolver();
        Self {
            harness: harness.map(|path| resolve_harness_api_with(path, resolver)),
            attributes: attributes.map(|path| resolve_harness_api_with(path, resolver)),
            state: None,
        }
    }

    /// Classify the explicit paths and decide the findings.
    fn resolve(
        explicit_harness: Option<&syn::Path>,
        effective_harness: Option<&syn::Path>,
        attributes: Option<&syn::Path>,
        shape: Shape,
    ) -> Self {
        let resolver = default_resolver();
        let harness = explicit_harness.map_or(SlotClass::Absent, |path| {
            classify_harness_path(path, resolver)
        });
        let attribute_slot = attributes.map_or(SlotClass::Absent, |path| {
            classify_policy_path(path, resolver)
        });
        let inputs = match shape {
            Shape::Scenario {
                is_async,
                has_executor_attr,
                has_gpui_test_attr,
            } => PolicyInputs::Scenario {
                harness,
                attributes: attribute_slot,
                is_async,
                has_executor_attr,
                has_gpui_test_attr,
            },
            Shape::Scenarios { runtime_alias } => PolicyInputs::Scenarios {
                harness,
                attributes: attribute_slot,
                runtime_alias,
            },
        };
        Self {
            harness: effective_harness.map(|path| resolve_harness_api_with(path, resolver)),
            attributes: attributes.map(|path| resolve_harness_api_with(path, resolver)),
            state: Some(DiagnosticState {
                inputs,
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
        self.state.as_ref().and_then(render::errors)
    }

    /// Emit the warnings this boundary owns.
    ///
    /// Call this at most once per boundary and only when [`Self::errors`] is
    /// `None`, because nightly emission is a side effect.
    pub(crate) fn emit_warnings(&self) -> TokenStream2 {
        let Some(state) = self.state.as_ref() else {
            return TokenStream2::new();
        };
        let warnings = render::warnings(state);
        emit::emit_native(&warnings);
        emit::warning_tokens(&warnings)
    }
}

/// The terminal identifier of a supplied path.
fn terminal_ident(path: &syn::Path) -> Option<Ident> {
    path.segments.last().map(|segment| segment.ident.clone())
}
