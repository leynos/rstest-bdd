//! Helpers for placeholder count mismatch diagnostic tests.
//!
//! These helpers are shared across multiple test binaries. Each binary compiles
//! the support module independently, so functions used by `diagnostics_placeholder`
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

use rstest_bdd_server::handlers::compute_signature_mismatch_diagnostics;
use rstest_bdd_server::server::ServerState;
use tempfile::TempDir;

/// Helper to compute placeholder mismatch diagnostics for a Rust file.
#[expect(dead_code, reason = "only used by diagnostics_placeholder test binary")]
#[allow(unfulfilled_lint_expectations)]
pub fn compute_placeholder_diagnostics(
    state: &ServerState,
    dir: &TempDir,
    filename: impl AsRef<str>,
) -> Vec<lsp_types::Diagnostic> {
    let path = dir.path().join(filename.as_ref());
    compute_signature_mismatch_diagnostics(state, &path)
}
