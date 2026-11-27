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
mod message;
mod model;
mod resolution;

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;

use proc_macro2::{Ident, TokenStream as TokenStream2};

#[cfg(not(rstest_bdd_nightly))]
use self::emit::Marker;
pub(crate) use self::{
    classify::{CrateResolver, classify_harness_path, classify_policy_path, default_resolver},
    model::{Framework, PolicyFindings, SlotClass},
    resolution::selected_hint,
};
use self::{
    emit::RenderedWarning,
    model::{PolicyInputs, diagnose},
    resolution::emitted_attributes,
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

/// The shape one boundary diagnoses.
#[derive(Clone, Copy)]
enum Shape {
    /// A `#[scenario]` expansion.
    Scenario {
        /// Whether the annotated function is `async`.
        is_async: bool,
        /// Whether the function already carries an executor attribute.
        has_user_test_attr: bool,
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
        is_async: bool,
        has_user_test_attr: bool,
    ) -> Self {
        Self::resolve(
            harness,
            harness,
            attributes,
            Shape::Scenario {
                is_async,
                has_user_test_attr,
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
                has_user_test_attr,
            } => PolicyInputs::Scenario {
                harness,
                attributes: attribute_slot,
                is_async,
                has_user_test_attr,
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

    /// Emit the diagnostics this boundary owns.
    ///
    /// Call this only from the boundary that built the value: nightly emission
    /// is a side effect, so a second call would duplicate the warning.
    pub(crate) fn emit_diagnostics(&self) -> TokenStream2 {
        let Some(state) = self.state.as_ref() else {
            return TokenStream2::new();
        };
        let warnings = render(state);
        emit::emit_native(&warnings);
        emit::warning_tokens(&warnings)
    }
}

/// The terminal identifier of a supplied path.
fn terminal_ident(path: &syn::Path) -> Option<Ident> {
    path.segments.last().map(|segment| segment.ident.clone())
}

/// Turn the findings into channel-neutral warnings.
///
/// A finding can hold only for an evidenced slot, whose path is by definition
/// present; a missing slot therefore means the model and the stored inputs
/// disagree, and dropping the warning keeps the renderer total without a
/// panic.
fn render(state: &DiagnosticState) -> Vec<RenderedWarning> {
    [harness_warning(state), attributes_warning(state)]
        .into_iter()
        .flatten()
        .collect()
}

/// Build the harness slot's warning, W002 taking the place of W001.
fn harness_warning(state: &DiagnosticState) -> Option<RenderedWarning> {
    let ident = state.slots.harness.as_ref()?;
    let SlotClass::Evidenced(framework) = state.inputs.harness() else {
        return None;
    };
    let written = ident.to_string();
    let canonical_harness = canonical_adapter_path(state.resolver, framework, &written);
    let canonicalized = state
        .inputs
        .with_harness(SlotClass::Recognized(framework.into()));
    let dropped = lost_attributes(state.inputs, canonicalized);
    let message = if state.findings.missing_attribute_policy {
        let canonical_policy = canonical_policy_path(state.resolver, framework);
        message::missing_attribute_policy(&canonical_harness, &canonical_policy, &dropped)
    } else {
        message::unrecognized_adapter_path(&written, &canonical_harness, &dropped)
    };
    #[cfg(not(rstest_bdd_nightly))]
    let marker = if state.findings.missing_attribute_policy {
        Marker::MissingAttributePolicy
    } else {
        Marker::UnrecognizedAdapterPath
    };
    Some(RenderedWarning {
        message,
        span: ident.span(),
        #[cfg(not(rstest_bdd_nightly))]
        marker,
    })
}

/// Build the attributes slot's warning, which is always W001.
fn attributes_warning(state: &DiagnosticState) -> Option<RenderedWarning> {
    let ident = state.slots.attributes.as_ref()?;
    let SlotClass::Evidenced(framework) = state.inputs.attributes() else {
        return None;
    };
    let written = ident.to_string();
    let canonical = canonical_adapter_path(state.resolver, framework, &written);
    let canonicalized = state
        .inputs
        .with_attributes(SlotClass::Recognized(framework.into()));
    let dropped = lost_attributes(state.inputs, canonicalized);
    Some(RenderedWarning {
        message: message::unrecognized_adapter_path(&written, &canonical, &dropped),
        span: ident.span(),
        #[cfg(not(rstest_bdd_nightly))]
        marker: Marker::UnrecognizedAdapterPath,
    })
}

/// The attributes the canonical spelling adds and the actual expansion omits.
///
/// A user-written executor attribute suppresses every "was not applied" claim,
/// because the generator deduplicates against it (decision D31).
fn lost_attributes(actual: PolicyInputs, canonical: PolicyInputs) -> Vec<&'static str> {
    if actual.has_user_test_attr() {
        return Vec::new();
    }
    emitted_attributes(canonical).difference(emitted_attributes(actual))
}

/// A resolver-rooted canonical adapter path for the type the user wrote.
fn canonical_adapter_path(
    resolver: &dyn CrateResolver,
    framework: Framework,
    written: &str,
) -> String {
    format!("{}::{written}", canonical_root(resolver, framework))
}

/// A resolver-rooted canonical attribute-policy path for one framework.
fn canonical_policy_path(resolver: &dyn CrateResolver, framework: Framework) -> String {
    format!(
        "{}::{}",
        canonical_root(resolver, framework),
        framework.attribute_policy_type()
    )
}

/// The crate root a suggested path must use.
///
/// The resolver's answer keeps a renamed dependency compilable: a crate that
/// depends on `tok = { package = "rstest-bdd-harness-tokio" }` must be told to
/// write `tok::TokioHarness`, not the name it does not have.
fn canonical_root(resolver: &dyn CrateResolver, framework: Framework) -> String {
    let spec = classify::adapter_spec(framework);
    resolver.resolved_root(spec.package_name).map_or_else(
        || spec.default_crate_name.to_owned(),
        |ident| ident.to_string(),
    )
}
