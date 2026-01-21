//! Diagnostic publishing via LSP.
//!
//! This module handles publishing diagnostics to the LSP client via
//! `textDocument/publishDiagnostics` notifications.

use std::path::Path;

use async_lsp::lsp_types::notification;
use lsp_types::{PublishDiagnosticsParams, Url};
use tracing::{debug, warn};

use crate::server::ServerState;

use super::compute::{compute_unimplemented_step_diagnostics, compute_unused_step_diagnostics};
use super::placeholder::compute_signature_mismatch_diagnostics;
use super::scenario_outline::compute_scenario_outline_column_diagnostics;
use super::table_docstring::compute_table_docstring_mismatch_diagnostics;

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
    let Some(client) = state.client() else {
        debug!("no client socket available for publishing diagnostics");
        return;
    };

    let Some(feature_index) = state.feature_index(feature_path) else {
        debug!(path = %feature_path.display(), "no feature index for diagnostics");
        return;
    };

    let Ok(uri) = Url::from_file_path(feature_path) else {
        warn!(path = %feature_path.display(), "cannot convert path to URI");
        return;
    };

    let mut diagnostics = compute_unimplemented_step_diagnostics(state, feature_index);
    diagnostics.extend(compute_table_docstring_mismatch_diagnostics(
        state,
        feature_index,
    ));
    diagnostics.extend(compute_scenario_outline_column_diagnostics(feature_index));

    let params = PublishDiagnosticsParams::new(uri, diagnostics, None);
    if let Err(err) = client.notify::<notification::PublishDiagnostics>(params) {
        warn!(error = %err, "failed to publish feature diagnostics");
    }
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
    let Some(client) = state.client() else {
        debug!("no client socket available for publishing diagnostics");
        return;
    };

    let Ok(uri) = Url::from_file_path(rust_path) else {
        warn!(path = %rust_path.display(), "cannot convert path to URI");
        return;
    };

    let mut diagnostics = compute_unused_step_diagnostics(state, rust_path);
    diagnostics.extend(compute_signature_mismatch_diagnostics(state, rust_path));

    let params = PublishDiagnosticsParams::new(uri, diagnostics, None);
    if let Err(err) = client.notify::<notification::PublishDiagnostics>(params) {
        warn!(error = %err, "failed to publish rust diagnostics");
    }
}
