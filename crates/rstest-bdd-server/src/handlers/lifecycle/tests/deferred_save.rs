//! Regression coverage for did-save work deferred during workspace preparation.

use lsp_types::{DidSaveTextDocumentParams, TextDocumentIdentifier, Url};

use super::super::{WorkspaceReadyEvent, handle_workspace_ready, prepare_workspace};
use super::cargo_workspace;
use crate::config::ServerConfig;
use crate::handlers::handle_did_save_text_document;
use crate::server::ServerState;

#[test]
fn workspace_ready_replays_a_deferred_disk_backed_feature_save() {
    let workspace = cargo_workspace().expect("create Cargo workspace");
    let feature_path = workspace.path().join("deferred.feature");
    std::fs::write(&feature_path, "Feature: deferred save\n").expect("write feature file");
    let feature_uri = Url::from_file_path(&feature_path).expect("feature URI");
    let mut state = ServerState::new(ServerConfig::default());
    let workspace_initialization_id = state.begin_workspace_initialization(Vec::new(), true);

    handle_did_save_text_document(
        &mut state,
        DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: feature_uri },
            text: None,
        },
    );

    assert_eq!(state.deferred_document_save_count(), 1);
    assert!(state.feature_index(&feature_path).is_none());

    handle_workspace_ready(
        &mut state,
        WorkspaceReadyEvent {
            preparation: prepare_workspace(workspace.path()),
            initialization_id: workspace_initialization_id,
        },
    );

    assert_eq!(state.deferred_document_save_count(), 0);
    assert!(state.feature_index(&feature_path).is_some());
}
