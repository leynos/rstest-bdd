//! Rendering tests for policy diagnostics.
//!
//! These pin what a user actually reads, independently of the channel that
//! carries it. They are also the consumer that keeps a nightly test build
//! honest: on nightly the native diagnostic API cannot be called from a test
//! process and the stable token builder is deliberately exclusive to stable,
//! so a rendered warning would otherwise exist with nothing reading it.

use proc_macro2::Span;

#[cfg(not(rstest_bdd_nightly))]
use super::super::emit::Marker;
use super::super::{
    DiagnosticState,
    PolicyInputs,
    SlotSpans,
    emit::RenderedWarning,
    model::{Framework, SlotClass, diagnose},
    render,
};

/// Build the diagnostic state for one input pair, spanned at `span`.
fn state(harness: SlotClass, attributes: SlotClass, span: Span) -> DiagnosticState {
    let inputs = PolicyInputs {
        harness,
        attributes,
    };
    DiagnosticState {
        inputs,
        spans: SlotSpans {
            harness: Some(span),
            attributes: Some(span),
        },
        findings: diagnose(inputs),
    }
}

/// Return the single rendered warning, or fail with a diagnostic.
fn only_warning(warnings: Vec<RenderedWarning>) -> RenderedWarning {
    let mut warnings = warnings.into_iter();
    let Some(warning) = warnings.next() else {
        panic!("an evidenced slot must yield one warning");
    };
    assert!(
        warnings.next().is_none(),
        "exactly one warning must be rendered"
    );
    warning
}

/// Assert that the warning points at the supplied span.
fn assert_spans(warning: &RenderedWarning, span: Span) {
    assert_eq!(
        warning.span.start(),
        span.start(),
        "the warning must start where the supplied path starts"
    );
    assert_eq!(
        warning.span.end(),
        span.end(),
        "the warning must end where the supplied path ends"
    );
}

#[test]
fn the_harness_warning_names_the_canonical_tokio_path() {
    let span = Span::call_site();
    let warning = only_warning(render(&state(
        SlotClass::Evidenced(Framework::Tokio),
        SlotClass::Absent,
        span,
    )));

    assert!(
        warning.message.contains("rstest-bdd-harness-tokio"),
        "the message must name the canonical crate: {}",
        warning.message
    );
    assert!(
        warning.message.contains("rstest_bdd_harness_tokio"),
        "the message must name the resolvable crate root: {}",
        warning.message
    );
    assert_spans(&warning, span);
    #[cfg(not(rstest_bdd_nightly))]
    assert_eq!(
        warning.marker,
        Marker::UnrecognizedAdapterPath,
        "the harness path uses the headline marker"
    );
}

#[test]
fn the_attributes_warning_names_the_canonical_gpui_crate() {
    let span = Span::call_site();
    let warning = only_warning(render(&state(
        SlotClass::Absent,
        SlotClass::Evidenced(Framework::Gpui),
        span,
    )));

    assert!(
        warning.message.contains("rstest-bdd-harness-gpui"),
        "the message must name the canonical crate: {}",
        warning.message
    );
    assert_spans(&warning, span);
}

#[test]
fn canonical_and_custom_paths_render_nothing() {
    let span = Span::call_site();
    let quiet = [
        SlotClass::Absent,
        SlotClass::Recognized(rstest_bdd_policy::TestAttributeHint::RstestOnly),
        SlotClass::Foreign,
    ];

    for harness in quiet {
        for attributes in quiet {
            assert!(
                render(&state(harness, attributes, span)).is_empty(),
                "no warning may be rendered for {harness:?} / {attributes:?}"
            );
        }
    }
}

#[test]
fn both_slots_render_one_warning_each() {
    let span = Span::call_site();
    let warnings = render(&state(
        SlotClass::Evidenced(Framework::Tokio),
        SlotClass::Evidenced(Framework::Tokio),
        span,
    ));

    assert_eq!(
        warnings.len(),
        2,
        "two evidenced slots yield one warning each"
    );
    assert!(
        warnings
            .iter()
            .all(|warning| warning.message.contains("rstest-bdd-harness-tokio")),
        "each warning names its own slot's canonical crate"
    );
}
