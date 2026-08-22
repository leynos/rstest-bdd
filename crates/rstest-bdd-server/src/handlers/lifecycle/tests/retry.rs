//! Regression coverage for retrying workspace preparation with pending saves.

use lsp_types::{
    DidSaveTextDocumentParams, InitializeParams, TextDocumentIdentifier, Url, WorkspaceFolder,
};

use super::super::handle_initialise;
use super::cargo_workspace;
use crate::config::ServerConfig;
use crate::handlers::handle_did_save_text_document;
use crate::server::ServerState;

#[test]
fn initialization_retry_clears_deferred_saves() {
    let workspace = cargo_workspace().expect("create Cargo workspace");
    let workspace_uri = Url::from_file_path(workspace.path()).expect("workspace URI");
    let deferred_uri = Url::from_file_path(workspace.path().join("deferred.feature"))
        .expect("deferred feature URI");
    let mut state = ServerState::new(ServerConfig::default());

    handle_initialise(
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

    assert_eq!(state.deferred_document_save_count(), 0);
}
