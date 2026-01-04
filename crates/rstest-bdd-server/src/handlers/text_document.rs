//! Text document notification handlers.
//!
//! Phase 7 focuses on building language-server foundations. This module
//! provides the on-save indexing pipeline for `.feature` files and Rust step
//! definition sources. Indexing results are stored in the shared server state.
//! After indexing, diagnostics are computed and published via the LSP protocol.

use lsp_types::DidSaveTextDocumentParams;
use tracing::{debug, warn};

use crate::indexing::{
    index_feature_file, index_feature_source, index_rust_file, index_rust_source,
};
use crate::server::ServerState;

use super::diagnostics::{
    publish_all_feature_diagnostics, publish_feature_diagnostics, publish_rust_diagnostics,
};

/// Handle `textDocument/didSave` notifications.
///
/// When a saved document is a `.feature` file or a Rust source file, the
/// server parses and indexes it. After successful indexing, diagnostics are
/// computed and published. Parse failures are logged but do not produce
/// diagnostics (the file remains in its previously indexed state).
pub fn handle_did_save_text_document(state: &mut ServerState, params: DidSaveTextDocumentParams) {
    let uri = params.text_document.uri;
    let Ok(path) = uri.to_file_path() else {
        debug!(%uri, "ignoring didSave for non-file URI");
        return;
    };

    if is_feature_file_path(&path) {
        handle_feature_file_save(state, &path, params.text.as_deref());
    } else if is_rust_file_path(&path) {
        handle_rust_file_save(state, &path, params.text.as_deref());
    }
}

fn handle_feature_file_save(state: &mut ServerState, path: &std::path::Path, text: Option<&str>) {
    let index_result = text.map_or_else(
        || index_feature_file(path),
        |source| index_feature_source(path.to_path_buf(), source),
    );

    match index_result {
        Ok(index) => {
            debug!(
                path = %path.display(),
                steps = index.steps.len(),
                examples = index.example_columns.len(),
                "indexed feature file"
            );
            state.upsert_feature_index(index);
            // Publish diagnostics for this feature file
            publish_feature_diagnostics(state, path);
        }
        Err(err) => {
            warn!(path = %path.display(), error = %err, "failed to index feature file");
        }
    }
}

fn is_feature_file_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|ext| ext.eq_ignore_ascii_case("feature"))
}

fn is_rust_file_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|ext| ext.eq_ignore_ascii_case("rs"))
}

fn handle_rust_file_save(state: &mut ServerState, path: &std::path::Path, text: Option<&str>) {
    let index_result = text.map_or_else(
        || index_rust_file(path),
        |source| index_rust_source(path.to_path_buf(), source),
    );

    match index_result {
        Ok(index) => {
            debug!(
                path = %path.display(),
                steps = index.step_definitions.len(),
                "indexed rust step file"
            );
            state.upsert_rust_step_index(index);
            // Rust file changes may affect all feature file diagnostics
            publish_all_feature_diagnostics(state);
            // Also check for unused step definitions in this file
            publish_rust_diagnostics(state, path);
        }
        Err(err) => {
            warn!(path = %path.display(), error = %err, "failed to index rust step file");
        }
    }
}
