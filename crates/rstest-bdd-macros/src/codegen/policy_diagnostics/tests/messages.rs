//! Rendering tests for policy diagnostics.
//!
//! These pin what a user actually reads, independently of the channel that
//! carries it. They are also the consumer that keeps a nightly test build
//! honest: on nightly the native diagnostic API cannot be called from a test
//! process and the stable token builder is deliberately exclusive to stable,
//! so a rendered warning would otherwise exist with nothing reading it.

use proc_macro2::{Ident, Span};

#[cfg(not(rstest_bdd_nightly))]
use super::super::emit::Marker;
use super::super::{
    DiagnosticState,
    PolicyInputs,
    SlotIdents,
    default_resolver,
    emit::RenderedWarning,
    model::{Framework, SlotClass, diagnose},
    render::warnings as render,
};

/// Build a state for one input pair, with both slots spelled `written`.
fn state(inputs: PolicyInputs, written: &str) -> DiagnosticState {
    let ident = Ident::new(written, Span::call_site());
    DiagnosticState {
        inputs,
        slots: SlotIdents {
            harness: Some(ident.clone()),
            attributes: Some(ident),
        },
        findings: diagnose(inputs),
        resolver: default_resolver(),
    }
}

/// A synchronous scenario with the supplied slots.
fn scenario(harness: SlotClass, attributes: SlotClass) -> PolicyInputs {
    PolicyInputs::Scenario {
        harness,
        attributes,
        is_async: false,
        has_user_test_attr: false,
    }
}

/// The single rendered warning, or a failure naming how many arrived.
fn only_warning(state: &DiagnosticState) -> RenderedWarning {
    let mut warnings = render(state).into_iter();
    let Some(warning) = warnings.next() else {
        panic!("an evidenced slot must yield one warning");
    };
    assert!(
        warnings.next().is_none(),
        "exactly one warning must be rendered"
    );
    warning
}

#[test]
fn the_harness_warning_names_the_canonical_tokio_path() {
    let state = state(
        scenario(SlotClass::Evidenced(Framework::Tokio), SlotClass::Absent),
        "TokioHarness",
    );
    let warning = only_warning(&state);

    assert!(
        warning
            .message
            .starts_with("rstest-bdd[W001]: this path was not recognized"),
        "the message carries its stable code: {}",
        warning.message
    );
    assert!(
        warning
            .message
            .contains("rstest_bdd_harness_tokio::TokioHarness`"),
        "the fix names the canonical path: {}",
        warning.message
    );
    assert!(
        !warning.message.contains("was not applied"),
        "a synchronous Tokio path loses no attribute: {}",
        warning.message
    );
    #[cfg(not(rstest_bdd_nightly))]
    assert_eq!(
        warning.marker,
        Marker::UnrecognizedAdapterPath,
        "W001 uses its own headline marker"
    );
}

#[test]
fn the_attributes_warning_names_the_dropped_gpui_attribute() {
    let state = state(
        scenario(SlotClass::Absent, SlotClass::Evidenced(Framework::Gpui)),
        "GpuiAttributePolicy",
    );
    let warning = only_warning(&state);

    assert!(
        warning.message.contains("`#[gpui::test]` was not applied"),
        "the message names the attribute that was lost: {}",
        warning.message
    );
    assert!(
        warning
            .message
            .contains("rstest_bdd_harness_gpui::GpuiAttributePolicy`"),
        "the fix names the canonical policy: {}",
        warning.message
    );
}

#[test]
fn the_missing_policy_warning_offers_both_fixes() {
    let state = state(
        scenario(SlotClass::Evidenced(Framework::Gpui), SlotClass::Absent),
        "GpuiHarness",
    );
    let warning = only_warning(&state);

    assert!(
        warning
            .message
            .starts_with("rstest-bdd[W002]: no attribute policy could be inferred"),
        "the message carries its stable code: {}",
        warning.message
    );
    assert!(
        warning.message.contains("`#[gpui::test]` was not applied"),
        "the message names the lost attribute: {}",
        warning.message
    );
    assert!(
        warning
            .message
            .contains("Write `rstest_bdd_harness_gpui::GpuiHarness`"),
        "the first fix is the canonical harness: {}",
        warning.message
    );
    assert!(
        warning
            .message
            .contains("attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy`"),
        "the second fix adds the policy explicitly: {}",
        warning.message
    );
    assert!(
        warning.message.contains("leaves warning W001"),
        "the second fix is honest about what it leaves behind: {}",
        warning.message
    );
    #[cfg(not(rstest_bdd_nightly))]
    assert_eq!(
        warning.marker,
        Marker::MissingAttributePolicy,
        "W002 uses its own headline marker"
    );
}

#[test]
fn a_user_executor_attribute_removes_the_attribute_claim() {
    let inputs = PolicyInputs::Scenario {
        harness: SlotClass::Evidenced(Framework::Gpui),
        attributes: SlotClass::Absent,
        is_async: false,
        has_user_test_attr: true,
    };
    let warning = only_warning(&state(inputs, "GpuiHarness"));

    assert!(
        warning.message.starts_with("rstest-bdd[W001]"),
        "the claim is downgraded to W001: {}",
        warning.message
    );
    assert!(
        !warning.message.contains("was not applied"),
        "a user-written attribute makes the claim false: {}",
        warning.message
    );
}

#[test]
fn the_warning_span_points_at_the_written_identifier() {
    let state = state(
        scenario(SlotClass::Evidenced(Framework::Tokio), SlotClass::Absent),
        "TokioHarness",
    );
    let warning = only_warning(&state);
    let expected = Ident::new("TokioHarness", Span::call_site()).span();

    assert_eq!(
        warning.span.start(),
        expected.start(),
        "the warning must start where the written identifier starts"
    );
    assert_eq!(
        warning.span.end(),
        expected.end(),
        "the warning must end where the written identifier ends"
    );
}

#[test]
fn canonical_and_custom_paths_render_nothing() {
    let quiet = [
        SlotClass::Absent,
        SlotClass::Recognized(rstest_bdd_policy::TestAttributeHint::RstestOnly),
        SlotClass::Foreign,
    ];

    for harness in quiet {
        for attributes in quiet {
            assert!(
                render(&state(scenario(harness, attributes), "TokioHarness")).is_empty(),
                "no warning may be rendered for {harness:?} / {attributes:?}"
            );
        }
    }
}

#[test]
fn both_slots_render_one_warning_each() {
    let state = state(
        scenario(
            SlotClass::Evidenced(Framework::Tokio),
            SlotClass::Evidenced(Framework::Tokio),
        ),
        "TokioAttributePolicy",
    );
    let warnings = render(&state);

    assert_eq!(
        warnings.len(),
        2,
        "two evidenced slots yield one warning each"
    );
    assert!(
        warnings
            .iter()
            .all(|warning| warning.message.contains("rstest-bdd[W001]")),
        "neither slot claims a missing policy when the policy is present"
    );
}
