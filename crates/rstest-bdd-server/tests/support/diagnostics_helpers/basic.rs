//! Helpers for basic diagnostic tests (unimplemented steps, unused definitions).
//!
//! These helpers are shared across multiple test binaries. Each binary compiles
//! the support module independently, so functions used by `diagnostics_basic`
//! appear as dead code when compiling other test binaries. The
//! `unfulfilled_lint_expectations` allow handles the reverse case where the
//! function IS used and the `expect(dead_code)` would otherwise fail.

// Allow `#[allow]` for `unfulfilled_lint_expectations` - this is unavoidable because:
// 1. `#[expect(dead_code)]` fails when the function IS used (unfulfilled expectation)
// 2. Each test binary compiles these helpers independently
// 3. We can't use `#[expect(unfulfilled_lint_expectations)]` because it too would fail
#![allow(
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    reason = "required for unfulfilled_lint_expectations workaround"
)]

use rstest_bdd_server::handlers::{
    compute_unimplemented_step_diagnostics, compute_unused_step_diagnostics,
};
use rstest_bdd_server::server::ServerState;
use tempfile::TempDir;

/// Helper to compute unimplemented step diagnostics for a feature file.
#[expect(dead_code, reason = "only used by diagnostics_basic test binary")]
#[allow(unfulfilled_lint_expectations)]
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
#[expect(dead_code, reason = "only used by diagnostics_basic test binary")]
#[allow(unfulfilled_lint_expectations)]
pub fn compute_rust_diagnostics(
    state: &ServerState,
    dir: &TempDir,
    filename: impl AsRef<str>,
) -> Vec<lsp_types::Diagnostic> {
    let path = dir.path().join(filename.as_ref());
    compute_unused_step_diagnostics(state, &path)
}

/// Helper to assert a single diagnostic with an expected message substring.
#[expect(dead_code, reason = "only used by diagnostics_basic test binary")]
#[allow(unfulfilled_lint_expectations)]
#[expect(clippy::expect_used, reason = "test helper uses expect for clarity")]
fn assert_single_diagnostic_contains(
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
#[expect(dead_code, reason = "only used by diagnostics_basic test binary")]
#[allow(unfulfilled_lint_expectations)]
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
#[expect(dead_code, reason = "only used by diagnostics_basic test binary")]
#[allow(unfulfilled_lint_expectations)]
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
#[expect(dead_code, reason = "only used by diagnostics_basic test binary")]
#[allow(unfulfilled_lint_expectations)]
pub fn assert_feature_has_no_diagnostics(state: &ServerState, dir: &TempDir, filename: &str) {
    let diagnostics = compute_feature_diagnostics(state, dir, filename);
    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, found {}",
        diagnostics.len()
    );
}

/// Helper to assert a rust file has no diagnostics.
#[expect(dead_code, reason = "only used by diagnostics_basic test binary")]
#[allow(unfulfilled_lint_expectations)]
pub fn assert_rust_has_no_diagnostics(state: &ServerState, dir: &TempDir, filename: &str) {
    let diagnostics = compute_rust_diagnostics(state, dir, filename);
    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, found {}",
        diagnostics.len()
    );
}
