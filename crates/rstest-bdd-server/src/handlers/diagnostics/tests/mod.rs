//! Shared fixtures and assertion helpers for diagnostics tests.

use std::path::Path;

use lsp_types::{Diagnostic, DiagnosticSeverity};
use rstest::{fixture, rstest};

use super::{compute::step_type_to_attribute, *};
use crate::{
    server::ServerState,
    test_support::{DiagnosticCheckType, ScenarioBuilder},
};

/// Fixture providing the infrastructure for diagnostic tests.
#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn scenario_builder() -> ScenarioBuilder { ScenarioBuilder::new() }

/// Helper to compute feature diagnostics for a path.
fn compute_feature_diagnostics_for_path(
    state: &ServerState,
    feature_path: &Path,
) -> Vec<Diagnostic> {
    let Some(feature_index) = state.feature_index(feature_path) else {
        panic!("feature index missing for {}", feature_path.display());
    };
    compute_unimplemented_step_diagnostics(state, feature_index)
}

fn assert_feature_has_no_unimplemented_steps(state: &ServerState, feature_path: &Path) {
    let diags = compute_feature_diagnostics_for_path(state, feature_path);
    assert!(
        diags.is_empty(),
        "expected no unimplemented steps, found {}",
        diags.len()
    );
}

fn assert_rust_has_no_unused_steps(state: &ServerState, rust_path: &Path) {
    let diags = compute_unused_step_diagnostics(state, rust_path);
    assert!(
        diags.is_empty(),
        "expected no unused step definitions, found {}",
        diags.len()
    );
}

/// Asserts exactly one diagnostic exists with the expected code and returns it.
fn assert_single_diagnostic_with_code<'a>(
    diagnostics: &'a [Diagnostic],
    expected_code: &str,
) -> &'a Diagnostic {
    let [diag] = diagnostics else {
        panic!("expected exactly 1 diagnostic, found {}", diagnostics.len());
    };
    assert_eq!(
        diag.code,
        Some(lsp_types::NumberOrString::String(expected_code.to_owned())),
        "expected diagnostic code '{expected_code}'"
    );
    diag
}

/// Asserts all fragments appear in the diagnostic message.
fn assert_diagnostic_message_contains(diag: &Diagnostic, fragments: &[&str]) {
    for fragment in fragments {
        assert!(
            diag.message.contains(fragment),
            "diagnostic message should contain '{fragment}', got: {}",
            diag.message
        );
    }
}

mod basic;
mod outline;
mod signature;
mod table_docstring;
