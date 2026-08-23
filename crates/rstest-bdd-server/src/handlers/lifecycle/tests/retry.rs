//! Regression coverage for retrying workspace preparation with pending saves.

use lsp_types::{
    DidSaveTextDocumentParams, InitializeParams, TextDocumentIdentifier, Url, WorkspaceFolder,
};
use metrics::with_local_recorder;

use super::super::{
    WorkspaceReadyEvent, handle_initialise, handle_workspace_ready, prepare_workspace,
};
use super::cargo_workspace;
use crate::config::ServerConfig;
use crate::handlers::handle_did_save_text_document;
use crate::handlers::workspace_metrics::WorkspaceRecorder;
use crate::server::ServerState;

#[test]
fn initialization_retry_clears_deferred_saves() {
    let workspace = cargo_workspace().expect("create Cargo workspace");
    let workspace_uri = Url::from_file_path(workspace.path()).expect("workspace URI");
    let deferred_uri = Url::from_file_path(workspace.path().join("deferred.feature"))
        .expect("deferred feature URI");
    let mut state = ServerState::new(ServerConfig::default());
    let recorder = WorkspaceRecorder::default();

    let first_initialization = with_local_recorder(&recorder, || {
        let initialization = handle_initialise(
            &mut state,
            InitializeParams {
                workspace_folders: Some(vec![WorkspaceFolder {
                    uri: workspace_uri,
                    name: "workspace".to_owned(),
                }]),
                ..Default::default()
            },
        )
        .expect("initialization should start workspace preparation");
        handle_did_save_text_document(
            &mut state,
            DidSaveTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: deferred_uri },
                text: None,
            },
        );
        assert_eq!(state.deferred_document_save_count(), 1);

        handle_initialise(&mut state, InitializeParams::default())
            .expect("retry should replace workspace preparation");
        initialization
    });

    assert_eq!(state.deferred_document_save_count(), 0);
    assert_eq!(
        recorder.workspace_outcome_count("deferred-save", "retry-clear"),
        1
    );
    assert_eq!(recorder.deferred_save_depth(), Some(0.0));
    handle_workspace_ready(
        &mut state,
        WorkspaceReadyEvent {
            preparation: prepare_workspace(workspace.path()),
            initialization_id: first_initialization.workspace_initialization_id,
        },
    );
    assert!(state.workspace_info().is_none());
}
