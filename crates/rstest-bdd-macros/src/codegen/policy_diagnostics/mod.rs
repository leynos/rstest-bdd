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
//! [`diagnose`](model::diagnose) decides the findings, and [`emit`] renders
//! them. Resolution is pure and side-effect free: only the owning boundary
//! calls [`SharedPolicyDiagnostics::emit_diagnostics`], and `scenarios!` calls
//! it once however many tests it generates, so the cardinality contract is
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

use proc_macro2::{Span, TokenStream as TokenStream2};

#[cfg(test)]
pub(crate) use self::model::Framework;
pub(crate) use self::{
    classify::{CrateResolver, classify_harness_path, classify_policy_path, default_resolver},
    model::{PolicyFindings, SlotClass},
    resolution::selected_hint,
};
use self::{
    emit::{Marker, RenderedWarning},
    model::{PolicyInputs, diagnose},
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
#[derive(Clone, Copy)]
struct DiagnosticState {
    /// The explicit user arguments, already classified.
    inputs: PolicyInputs,
    /// The terminal identifier span of each explicit path.
    spans: SlotSpans,
    /// What the classifier found.
    findings: PolicyFindings,
}

/// The spans a rendered diagnostic can point at.
#[derive(Clone, Copy)]
struct SlotSpans {
    /// Span of the explicit `harness` path's terminal identifier.
    harness: Option<Span>,
    /// Span of the explicit `attributes` path's terminal identifier.
    attributes: Option<Span>,
}

impl SharedPolicyDiagnostics {
    /// Resolve the inputs of a `#[scenario]` expansion.
    pub(crate) fn for_scenario(
        harness: Option<&syn::Path>,
        attributes: Option<&syn::Path>,
    ) -> Self {
        Self::resolve(harness, harness, attributes)
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
    ) -> Self {
        Self::resolve(harness, effective_harness, attributes)
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
    ) -> Self {
        let resolver = default_resolver();
        let inputs = PolicyInputs {
            harness: explicit_harness.map_or(SlotClass::Absent, |path| {
                classify_harness_path(path, resolver)
            }),
            attributes: attributes.map_or(SlotClass::Absent, |path| {
                classify_policy_path(path, resolver)
            }),
        };
        Self {
            harness: effective_harness.map(|path| resolve_harness_api_with(path, resolver)),
            attributes: attributes.map(|path| resolve_harness_api_with(path, resolver)),
            state: Some(DiagnosticState {
                inputs,
                spans: SlotSpans {
                    harness: explicit_harness.and_then(terminal_ident_span),
                    attributes: attributes.and_then(terminal_ident_span),
                },
                findings: diagnose(inputs),
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

/// The span a generated marker points at.
fn terminal_ident_span(path: &syn::Path) -> Option<Span> {
    path.segments.last().map(|segment| segment.ident.span())
}

/// Turn the findings into channel-neutral warnings.
///
/// A finding can hold only for an evidenced slot, whose path is by definition
/// present, so a missing span or a non-evidenced class means the model and the
/// stored inputs disagree; dropping the warning keeps the renderer total
/// without a panic.
fn render(state: &DiagnosticState) -> Vec<RenderedWarning> {
    [
        (
            state.findings.harness_unrecognized_path,
            state.inputs.harness,
            state.spans.harness,
        ),
        (
            state.findings.attributes_unrecognized_path,
            state.inputs.attributes,
            state.spans.attributes,
        ),
    ]
    .into_iter()
    .filter(|(holds, ..)| *holds)
    .filter_map(|(_, class, span)| evidenced_warning(class, span))
    .collect()
}

/// Build the W001 warning for one evidenced slot.
fn evidenced_warning(class: SlotClass, span: Option<Span>) -> Option<RenderedWarning> {
    let SlotClass::Evidenced(framework) = class else {
        return None;
    };
    Some(RenderedWarning {
        message: message::unrecognized_adapter_path(framework),
        span: span?,
        marker: Marker::UnrecognizedAdapterPath,
    })
}
