//! Behavioural test for `.feature` file indexing on save.

use lsp_types::{DidSaveTextDocumentParams, TextDocumentIdentifier, Url};
use rstest_bdd_server::config::ServerConfig;
use rstest_bdd_server::discovery::WorkspaceInfo;
use rstest_bdd_server::handlers::handle_did_save_text_document;
use rstest_bdd_server::server::ServerState;
use tempfile::TempDir;

#[test]
fn did_save_indexes_feature_files_and_caches_result() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("demo.feature");
    std::fs::write(
        &path,
        concat!(
            "Feature: demo\n",
            "  Scenario: s\n",
            "    Given a message\n",
            "      \"\"\"\n",
            "      hello\n",
            "      \"\"\"\n",
        ),
    )
    .expect("write feature file");

    let uri = Url::from_file_path(&path).expect("file URI");
    let params = DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri },
        text: None,
    };

    let mut state = state_for_workspace(dir.path());
    handle_did_save_text_document(&mut state, params);

    let index = state.feature_index(&path).expect("feature index cached");
    assert_eq!(index.steps.len(), 1);
    let step = index.steps.first().expect("expected indexed step");
    assert!(step.docstring.is_some());
}

#[test]
fn did_save_normalizes_disk_feature_source() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("missing-newline.feature");
    std::fs::write(&path, "Feature: demo\n  Scenario: s\n    Given a message")
        .expect("write feature file");
    let uri = Url::from_file_path(&path).expect("file URI");
    let params = DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri },
        text: None,
    };

    let mut state = state_for_workspace(dir.path());
    handle_did_save_text_document(&mut state, params);

    let index = state.feature_index(&path).expect("feature index cached");
    assert!(index.source.ends_with('\n'));
}

#[test]
fn did_save_rejects_feature_files_outside_workspace_root() {
    let workspace = TempDir::new().expect("workspace dir");
    let outside_workspace = TempDir::new().expect("outside workspace dir");
    let path = outside_workspace.path().join("outside.feature");
    std::fs::write(
        &path,
        concat!(
            "Feature: outside\n",
            "  Scenario: s\n",
            "    Given a message\n",
        ),
    )
    .expect("write feature file");

    let uri = Url::from_file_path(&path).expect("file URI");
    let params = DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri },
        text: None,
    };

    let mut state = state_for_workspace(workspace.path());
    handle_did_save_text_document(&mut state, params);

    assert!(state.feature_index(&path).is_none());
}

#[test]
fn did_save_with_text_indexes_paths_outside_workspace_root() {
    let workspace = TempDir::new().expect("workspace dir");
    let outside_workspace = TempDir::new().expect("outside workspace dir");
    let path = outside_workspace.path().join("provided.feature");
    let source = concat!(
        "Feature: provided\n",
        "  Scenario: s\n",
        "    Given a message\n",
    )
    .to_owned();

    let uri = Url::from_file_path(&path).expect("file URI");
    let params = DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri },
        text: Some(source),
    };

    let mut state = state_for_workspace(workspace.path());
    handle_did_save_text_document(&mut state, params);

    assert!(state.feature_index(&path).is_some());
}

fn state_for_workspace(root: &std::path::Path) -> ServerState {
    let mut state = ServerState::new(ServerConfig::default());
    if let Err(error) = state.set_workspace_info(WorkspaceInfo {
        root: root.to_path_buf(),
        packages: Vec::new(),
    }) {
        panic!("configure workspace root: {error}");
    }
    state
}
