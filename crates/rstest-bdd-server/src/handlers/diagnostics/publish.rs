//! Diagnostic publishing via LSP.
//!
//! This module handles publishing diagnostics to the LSP client via
//! `textDocument/publishDiagnostics` notifications. All publishing flows
//! through the canonical [`publish_with`] boundary: one place owns the
//! client/URI guards, parameter construction, notification, and error
//! logging. The per-file-kind functions only differ in the diagnostics they
//! compute.

use std::path::Path;

use async_lsp::lsp_types::notification;
use lsp_types::{Diagnostic, PublishDiagnosticsParams, Url};
use tracing::{debug, warn};

use crate::server::ServerState;

use super::compute::{compute_unimplemented_step_diagnostics, compute_unused_step_diagnostics};
use super::placeholder::compute_signature_mismatch_diagnostics;
use super::scenario_outline::compute_scenario_outline_column_diagnostics;
use super::table_docstring::compute_table_docstring_mismatch_diagnostics;

/// Compute all diagnostics for a feature file, or `None` when the file has
/// no feature index (in which case nothing is published, preserving any
/// previously published diagnostics).
fn compute_feature_file_diagnostics(
    state: &ServerState,
    feature_path: &Path,
) -> Option<Vec<Diagnostic>> {
    let Some(feature_index) = state.feature_index(feature_path) else {
        debug!(path = %feature_path.display(), "no feature index for diagnostics");
        return None;
    };
    let mut diagnostics = compute_unimplemented_step_diagnostics(state, feature_index);
    diagnostics.extend(compute_table_docstring_mismatch_diagnostics(
        state,
        feature_index,
    ));
    diagnostics.extend(compute_scenario_outline_column_diagnostics(feature_index));
    Some(diagnostics)
}

/// Compute all diagnostics for a Rust step definition file.
///
/// An empty vector is still published so stale diagnostics are cleared.
fn compute_rust_file_diagnostics(state: &ServerState, rust_path: &Path) -> Vec<Diagnostic> {
    let mut diagnostics = compute_unused_step_diagnostics(state, rust_path);
    diagnostics.extend(compute_signature_mismatch_diagnostics(state, rust_path));
    diagnostics
}

/// Build the publish parameters for `path` from a computation, without
/// performing any I/O towards the client.
///
/// Returns `None` (publishing is skipped) when `compute` declines to produce
/// diagnostics or when `path` cannot be converted to a URI. An empty
/// diagnostics vector still yields parameters, because publishing an empty
/// array clears previously published diagnostics.
///
/// Separated from [`publish_with`] so tests can pin the published payload
/// without a client socket.
fn prepare_publish(
    state: &ServerState,
    path: &Path,
    compute: impl FnOnce(&ServerState, &Path) -> Option<Vec<Diagnostic>>,
) -> Option<PublishDiagnosticsParams> {
    let diagnostics = compute(state, path)?;
    let Ok(uri) = Url::from_file_path(path) else {
        warn!(path = %path.display(), "cannot convert path to URI");
        return None;
    };
    Some(PublishDiagnosticsParams::new(uri, diagnostics, None))
}

/// Canonical publish boundary: guard the client socket, build parameters via
/// [`prepare_publish`], send the notification, and log failures.
///
/// All diagnostic publishing must flow through this helper so the guards,
/// parameter construction, and error logging exist exactly once; see the
/// developers' guide for ownership and permitted call-sites.
fn publish_with(
    state: &ServerState,
    path: &Path,
    failure_message: &'static str,
    compute: impl FnOnce(&ServerState, &Path) -> Option<Vec<Diagnostic>>,
) {
    let Some(client) = state.client() else {
        debug!("no client socket available for publishing diagnostics");
        return;
    };
    let Some(params) = prepare_publish(state, path, compute) else {
        return;
    };
    if let Err(err) = client.notify::<notification::PublishDiagnostics>(params) {
        warn!(error = %err, "{}", failure_message);
    }
}

/// Publish diagnostics for a single feature file.
///
/// Computes diagnostics for:
/// - Unimplemented steps
/// - Table/docstring expectation mismatches
/// - Scenario outline column mismatches
///
/// Publishes them via the client socket. Publishes an empty array if no issues
/// are found, clearing any previous diagnostics.
pub fn publish_feature_diagnostics(state: &ServerState, feature_path: &Path) {
    publish_with(
        state,
        feature_path,
        "failed to publish feature diagnostics",
        compute_feature_file_diagnostics,
    );
}

/// Publish diagnostics for all feature files.
///
/// Called when a Rust file is saved, as step definition changes may affect
/// which feature steps are unimplemented.
pub fn publish_all_feature_diagnostics(state: &ServerState) {
    // Collect paths first to avoid borrowing issues
    let feature_paths: Vec<_> = state
        .all_feature_indices()
        .map(|index| index.path.clone())
        .collect();

    for path in feature_paths {
        publish_feature_diagnostics(state, &path);
    }
}

/// Publish diagnostics for Rust step definition files.
///
/// Computes diagnostics for:
/// - Unused step definitions
/// - Placeholder count mismatches
///
/// Publishes them via the client socket. Publishes an empty array if no issues
/// are found, clearing any previous diagnostics.
pub fn publish_rust_diagnostics(state: &ServerState, rust_path: &Path) {
    publish_with(
        state,
        rust_path,
        "failed to publish rust diagnostics",
        |state, path| Some(compute_rust_file_diagnostics(state, path)),
    );
}

#[cfg(test)]
mod tests {
    //! Snapshot and property tests for diagnostic publication payloads.

    use lsp_types::{DiagnosticSeverity, Position, Range};
    use proptest::prelude::*;

    use crate::config::ServerConfig;
    use crate::test_support::ScenarioBuilder;

    use super::*;

    const FEATURE_SOURCE: &str = concat!(
        "Feature: demo\n",
        "  Scenario: example\n",
        "    Given an implemented step\n",
        "    When an unimplemented step\n",
    );

    const RUST_SOURCE: &str = concat!(
        "use rstest_bdd_macros::{given, when};\n\n",
        "#[given(\"an implemented step\")]\n",
        "fn implemented(count: u32) {}\n\n",
        "#[when(\"an unused step\")]\n",
        "fn unused() {}\n",
    );

    /// Snapshot settings that normalize the temp-dir portion of file URIs.
    ///
    /// The filter matches the URL-rendered form of the directory (for
    /// example `/C:/Users/...` on Windows rather than `C:\Users\...`), as
    /// that is what the `PublishDiagnosticsParams` debug output contains.
    fn snapshot_settings(dir: &Path) -> insta::Settings {
        let mut settings = insta::Settings::clone_current();
        let Ok(dir_url) = Url::from_file_path(dir) else {
            panic!("temp dir should convert to a URL: {}", dir.display());
        };
        settings.add_filter(&regex::escape(dir_url.path()), "[TMP]");
        settings
    }

    #[test]
    fn feature_publish_payload_is_pinned() {
        let scenario = ScenarioBuilder::new().with_single_file_pair(FEATURE_SOURCE, RUST_SOURCE);
        let params = prepare_publish(
            &scenario.state,
            &scenario.feature_path,
            compute_feature_file_diagnostics,
        );
        #[expect(clippy::expect_used, reason = "feature index exists for staged file")]
        let params = params.expect("feature file publishes diagnostics");
        snapshot_settings(scenario.dir.path()).bind(|| {
            insta::assert_debug_snapshot!("feature_publish_params", params);
        });
    }

    #[test]
    fn rust_publish_payload_is_pinned() {
        let scenario = ScenarioBuilder::new().with_single_file_pair(FEATURE_SOURCE, RUST_SOURCE);
        let params = prepare_publish(&scenario.state, &scenario.rust_path, |state, path| {
            Some(compute_rust_file_diagnostics(state, path))
        });
        #[expect(clippy::expect_used, reason = "rust files always publish")]
        let params = params.expect("rust file publishes diagnostics");
        snapshot_settings(scenario.dir.path()).bind(|| {
            insta::assert_debug_snapshot!("rust_publish_params", params);
        });
    }

    #[test]
    fn missing_feature_index_skips_publishing() {
        let state = crate::server::ServerState::new(ServerConfig::default());
        let params = prepare_publish(
            &state,
            Path::new("/no/such/file.feature"),
            compute_feature_file_diagnostics,
        );
        assert!(
            params.is_none(),
            "missing feature index must skip publishing"
        );
    }

    /// Strategy producing an arbitrary diagnostic vector (including empty).
    fn diagnostics_strategy() -> impl Strategy<Value = Vec<Diagnostic>> {
        proptest::collection::vec(
            ("[a-zA-Z ]{1,30}", 0u32..500, 0u32..120).prop_map(|(message, line, col)| Diagnostic {
                range: Range::new(Position::new(line, col), Position::new(line, col + 1)),
                severity: Some(DiagnosticSeverity::WARNING),
                message,
                ..Diagnostic::default()
            }),
            0..8,
        )
    }

    proptest! {
        /// Publishing invariants: any computed vector (including empty) is
        /// published verbatim — same count, same order, same URI target.
        #[test]
        fn publish_params_preserve_computed_diagnostics(
            diagnostics in diagnostics_strategy(),
        ) {
            let state = crate::server::ServerState::new(ServerConfig::default());
            // `Url::from_file_path` requires a platform-absolute path, so a
            // hard-coded `/tmp/...` literal would fail on Windows.
            let path = std::env::temp_dir().join("publish-invariants.rs");
            let expected = diagnostics.clone();
            let params = prepare_publish(&state, &path, move |_, _| Some(diagnostics));
            let params = params.ok_or_else(|| {
                TestCaseError::fail("valid path must produce publish params")
            })?;
            prop_assert_eq!(&params.diagnostics, &expected);
            #[expect(clippy::expect_used, reason = "fixed absolute path converts")]
            let expected_uri = Url::from_file_path(&path).expect("uri");
            prop_assert_eq!(params.uri, expected_uri);
        }
    }
}
