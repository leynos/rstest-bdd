//! Regression coverage for stopped-router workspace event delivery.

use async_lsp::ClientSocket;
use lsp_types::{DidSaveTextDocumentParams, TextDocumentIdentifier, Url};
use metrics::with_local_recorder;

use super::super::{WorkspaceReadyEvent, emit_workspace_ready, prepare_workspace};
use super::cargo_workspace;
use crate::config::ServerConfig;
use crate::handlers::handle_did_save_text_document;
use crate::handlers::workspace_metrics::WorkspaceRecorder;
use crate::server::ServerState;

#[test]
fn workspace_ready_delivery_failure_keeps_deferred_saves_for_the_stopped_router() {
    let workspace = cargo_workspace().expect("create Cargo workspace");
    let deferred_uri = Url::from_file_path(workspace.path().join("deferred.feature"))
        .expect("deferred feature URI");
    let mut state = ServerState::new(ServerConfig::default());
    let workspace_initialization_id = state.begin_workspace_initialization(Vec::new(), true);
    handle_did_save_text_document(
        &mut state,
        DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: deferred_uri },
            text: None,
        },
    );
    let recorder = WorkspaceRecorder::default();

    with_local_recorder(&recorder, || {
        emit_workspace_ready(
            &ClientSocket::new_closed(),
            WorkspaceReadyEvent {
                preparation: prepare_workspace(workspace.path()),
                initialization_id: workspace_initialization_id,
            },
        );
    });

    assert_eq!(
        recorder.workspace_outcome_count("workspace-preparation", "event-delivery-failure"),
        1
    );
    assert_eq!(state.deferred_document_save_count(), 1);
    assert!(state.workspace_preparation_pending());
}
