//! Helper functions for diagnostic integration tests.
//!
//! These helpers are shared across multiple test binaries, but each binary
//! uses only a subset. The `dead_code` allow suppresses warnings for helpers
//! that aren't used in a particular binary.

#![allow(dead_code, reason = "each test binary uses a different subset of helpers")]

use rstest_bdd_server::handlers::diagnostics::compute::{
    compute_signature_mismatch_diagnostics, compute_table_docstring_mismatch_diagnostics,
};
use rstest_bdd_server::handlers::{
    compute_unimplemented_step_diagnostics, compute_unused_step_diagnostics,
};
use rstest_bdd_server::server::ServerState;
use tempfile::TempDir;

/// Helper to compute unimplemented step diagnostics for a feature file.
#[expect(clippy::expect_used, reason = "test helper uses expect for clarity")]
pub fn compute_feature_diagnostics(
    state: &ServerState,
    dir: &TempDir,
    filename: impl AsRef<str>,
) -> Vec<lsp_types::Diagnostic> {
    let path = dir.path().join(filename.as_ref());
    let feature_index = state.feature_index(&path).expect("feature index");
    compute_unimplemented_step_diagnostics(state, feature_index)
}

/// Helper to compute unused step definition diagnostics for a Rust file.
pub fn compute_rust_diagnostics(
    state: &ServerState,
    dir: &TempDir,
    filename: impl AsRef<str>,
) -> Vec<lsp_types::Diagnostic> {
    let path = dir.path().join(filename.as_ref());
    compute_unused_step_diagnostics(state, &path)
}

/// Helper to compute placeholder mismatch diagnostics for a Rust file.
pub fn compute_placeholder_diagnostics(
    state: &ServerState,
    dir: &TempDir,
    filename: impl AsRef<str>,
) -> Vec<lsp_types::Diagnostic> {
    let path = dir.path().join(filename.as_ref());
    compute_signature_mismatch_diagnostics(state, &path)
}

/// Helper to compute table/docstring mismatch diagnostics for a feature file.
#[expect(clippy::expect_used, reason = "test helper uses expect for clarity")]
pub fn compute_table_docstring_diagnostics(
    state: &ServerState,
    dir: &TempDir,
    filename: impl AsRef<str>,
) -> Vec<lsp_types::Diagnostic> {
    let path = dir.path().join(filename.as_ref());
    let feature_index = state.feature_index(&path).expect("feature index");
    compute_table_docstring_mismatch_diagnostics(state, feature_index)
}

/// Helper to assert a single diagnostic with an expected message substring.
#[expect(clippy::expect_used, reason = "test helper uses expect for clarity")]
pub fn assert_single_diagnostic_contains(
    diagnostics: &[lsp_types::Diagnostic],
    expected_substring: &str,
) {
    assert_eq!(diagnostics.len(), 1, "expected exactly one diagnostic");
    assert!(
        diagnostics
            .first()
            .expect("one diagnostic")
            .message
            .contains(expected_substring),
        "diagnostic message should contain '{expected_substring}'"
    );
}

/// Helper to assert a feature file has a diagnostic with expected message.
pub fn assert_feature_has_diagnostic(
    state: &ServerState,
    dir: &TempDir,
    filename: &str,
    message_substring: &str,
) {
    let diagnostics = compute_feature_diagnostics(state, dir, filename);
    assert_single_diagnostic_contains(&diagnostics, message_substring);
}

/// Helper to assert a rust file has a diagnostic with expected message.
pub fn assert_rust_has_diagnostic(
    state: &ServerState,
    dir: &TempDir,
    filename: &str,
    message_substring: &str,
) {
    let diagnostics = compute_rust_diagnostics(state, dir, filename);
    assert_single_diagnostic_contains(&diagnostics, message_substring);
}

/// Helper to assert a feature file has no diagnostics.
pub fn assert_feature_has_no_diagnostics(state: &ServerState, dir: &TempDir, filename: &str) {
    let diagnostics = compute_feature_diagnostics(state, dir, filename);
    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, found {}",
        diagnostics.len()
    );
}

/// Helper to assert a rust file has no diagnostics.
pub fn assert_rust_has_no_diagnostics(state: &ServerState, dir: &TempDir, filename: &str) {
    let diagnostics = compute_rust_diagnostics(state, dir, filename);
    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, found {}",
        diagnostics.len()
    );
}
