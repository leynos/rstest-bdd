//! Helpers for table/docstring expectation mismatch diagnostic tests.

use rstest_bdd_server::handlers::compute_table_docstring_mismatch_diagnostics;
use rstest_bdd_server::server::ServerState;
use tempfile::TempDir;

/// Helper to compute table/docstring mismatch diagnostics for a feature file.
#[allow(
    dead_code,
    reason = "only used by diagnostics_table_docstring test binary"
)]
#[expect(
    clippy::allow_attributes,
    reason = "expect fails when function is used"
)]
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
