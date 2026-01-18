//! Helpers for placeholder count mismatch diagnostic tests.

use rstest_bdd_server::handlers::compute_signature_mismatch_diagnostics;
use rstest_bdd_server::server::ServerState;
use tempfile::TempDir;

/// Helper to compute placeholder mismatch diagnostics for a Rust file.
#[allow(dead_code, reason = "only used by diagnostics_placeholder test binary")]
#[expect(
    clippy::allow_attributes,
    reason = "expect fails when function is used"
)]
pub fn compute_placeholder_diagnostics(
    state: &ServerState,
    dir: &TempDir,
    filename: impl AsRef<str>,
) -> Vec<lsp_types::Diagnostic> {
    let path = dir.path().join(filename.as_ref());
    compute_signature_mismatch_diagnostics(state, &path)
}
