//! Background indexing for did-save notifications held during preparation.
//!
//! The worker performs filesystem reads and parsing off the current-thread LSP
//! executor. Its event returns completed indexes to the router, where state
//! mutation and diagnostic publication remain serialized.

use std::path::PathBuf;

use async_lsp::ClientSocket;
use lsp_types::DidSaveTextDocumentParams;
use tracing::{Instrument, debug, info_span, warn};

use crate::indexing::{
    FeatureFileIndex, FeatureIndexError, RustStepIndexError, RustStepIndexResult, WorkspaceRoot,
    index_feature_source, index_feature_source_owned, index_rust_file, index_rust_source,
};
use crate::server::ServerState;

use super::diagnostics::{FeatureDiagnosticPublication, publish_all_feature_diagnostics};
use super::text_document::{
    apply_feature_index_result, apply_rust_index_result, index_saved_document,
};
use super::util::has_extension;
use super::workspace_metrics::{record_deferred_save_depth, record_workspace_outcome};

/// Completed indexes from a deferred did-save replay worker.
pub struct DeferredDocumentSavesIndexed {
    initialization_id: u64,
    results: Vec<DeferredDocumentSaveIndex>,
}

enum DeferredDocumentSaveIndex {
    /// A feature-file indexing result ready for router-owned application.
    Feature {
        path: PathBuf,
        result: Result<FeatureFileIndex, FeatureIndexError>,
    },
    /// A Rust-file indexing result ready for router-owned application.
    Rust {
        path: PathBuf,
        result: Result<RustStepIndexResult, RustStepIndexError>,
    },
}

/// Start a bounded deferred-save replay without blocking the router.
pub(crate) fn start_deferred_document_save_replay(
    state: &mut ServerState,
    initialization_id: u64,
    deferred_document_saves: Vec<DidSaveTextDocumentParams>,
) {
    record_deferred_save_depth(0);
    let Some(client) = state.client().cloned() else {
        for params in deferred_document_saves {
            record_workspace_outcome("deferred-save", "replayed");
            index_saved_document(state, params);
        }
        return;
    };
    let workspace_root = state.workspace_root_for_replay();
    let span = info_span!("deferred_save_replay", initialization_id);
    let task = tokio::spawn(
        async move {
            let results = tokio::task::spawn_blocking(move || {
                index_deferred_document_saves(deferred_document_saves, workspace_root.as_ref())
            })
            .await;
            match results {
                Ok(results) => emit_replayed_indexes(&client, initialization_id, results),
                Err(error) => {
                    record_workspace_outcome("deferred-save", "join-failure");
                    warn!(error = %error, "deferred document-save replay task failed");
                }
            }
        }
        .instrument(span),
    );
    state.replace_workspace_task(task);
}

fn emit_replayed_indexes(
    client: &ClientSocket,
    initialization_id: u64,
    results: Vec<DeferredDocumentSaveIndex>,
) {
    if let Err(error) = client.emit(DeferredDocumentSavesIndexed {
        initialization_id,
        results,
    }) {
        record_workspace_outcome("deferred-save", "event-delivery-failure");
        warn!(error = %error, event = "deferred-save-indexed", "failed to publish deferred indexes");
    }
}

/// Apply background deferred-save indexing results on the router task.
pub fn handle_deferred_document_saves_indexed(
    state: &mut ServerState,
    event: DeferredDocumentSavesIndexed,
) {
    if !state.is_current_workspace_initialization(event.initialization_id) {
        debug!(
            initialization_id = event.initialization_id,
            "discarding stale deferred-save replay"
        );
        return;
    }
    state.clear_workspace_task();
    for result in event.results {
        record_workspace_outcome("deferred-save", "replayed");
        match result {
            DeferredDocumentSaveIndex::Feature { path, result } => {
                apply_feature_index_result(
                    state,
                    &path,
                    result,
                    FeatureDiagnosticPublication::DeferredReplay,
                );
            }
            DeferredDocumentSaveIndex::Rust { path, result } => {
                apply_rust_index_result(
                    state,
                    &path,
                    result,
                    FeatureDiagnosticPublication::DeferredReplay,
                );
            }
        }
    }
    publish_all_feature_diagnostics(state);
}

fn index_deferred_document_saves(
    deferred_document_saves: Vec<DidSaveTextDocumentParams>,
    workspace_root: Option<&WorkspaceRoot>,
) -> Vec<DeferredDocumentSaveIndex> {
    deferred_document_saves
        .into_iter()
        .filter_map(|params| index_deferred_document_save(params, workspace_root))
        .collect()
}

fn index_deferred_document_save(
    params: DidSaveTextDocumentParams,
    workspace_root: Option<&WorkspaceRoot>,
) -> Option<DeferredDocumentSaveIndex> {
    let path = params.text_document.uri.to_file_path().ok()?;
    if has_extension(&path, "feature") {
        let result = params.text.map_or_else(
            || index_disk_backed_feature(&path, workspace_root),
            |source| index_feature_source(path.clone(), &source),
        );
        Some(DeferredDocumentSaveIndex::Feature { path, result })
    } else if has_extension(&path, "rs") {
        let result = params.text.map_or_else(
            || index_rust_file(&path),
            |source| index_rust_source(path.clone(), &source),
        );
        Some(DeferredDocumentSaveIndex::Rust { path, result })
    } else {
        None
    }
}

fn index_disk_backed_feature(
    path: &std::path::Path,
    workspace_root: Option<&WorkspaceRoot>,
) -> Result<FeatureFileIndex, FeatureIndexError> {
    let workspace_root = workspace_root.ok_or(FeatureIndexError::WorkspaceRootUnavailable)?;
    let source = workspace_root.read_feature_source(path)?;
    index_feature_source_owned(path.to_path_buf(), source)
}

#[cfg(test)]
mod tests {
    //! Tests for background deferred-save replay results.

    use std::future::pending;

    use async_lsp::ClientSocket;
    use metrics::with_local_recorder;

    use super::*;
    use crate::config::ServerConfig;
    use crate::handlers::workspace_metrics::WorkspaceRecorder;

    #[test]
    fn deferred_replay_delivery_failure_records_a_bounded_outcome() {
        let recorder = WorkspaceRecorder::default();

        with_local_recorder(&recorder, || {
            emit_replayed_indexes(&ClientSocket::new_closed(), 1, Vec::new());
        });

        assert_eq!(
            recorder.workspace_outcome_count("deferred-save", "event-delivery-failure"),
            1
        );
    }

    #[tokio::test]
    async fn stale_replay_does_not_replace_the_current_workspace_task() {
        let mut state = ServerState::new(ServerConfig::default());
        let stale_initialization_id = state.begin_workspace_initialization(Vec::new(), true);
        let current_initialization_id = state.begin_workspace_initialization(Vec::new(), true);
        let task = tokio::spawn(pending::<()>());
        state.replace_workspace_task(task);
        let path = PathBuf::from("stale.feature");
        let index = index_feature_source(path.clone(), "Feature: stale\n")
            .expect("feature source should index");

        handle_deferred_document_saves_indexed(
            &mut state,
            DeferredDocumentSavesIndexed {
                initialization_id: stale_initialization_id,
                results: vec![DeferredDocumentSaveIndex::Feature {
                    path: path.clone(),
                    result: Ok(index),
                }],
            },
        );

        assert!(state.is_current_workspace_initialization(current_initialization_id));
        assert!(state.feature_index(&path).is_none());
        let Some(task) = state.take_workspace_task() else {
            panic!("the current workspace task should remain retained");
        };
        task.abort();
        let Err(error) = task.await else {
            panic!("the pending workspace task should be cancelled");
        };
        assert!(error.is_cancelled());
    }
}
