//! Behavioural test for `.feature` file indexing on save.

use lsp_types::{DidSaveTextDocumentParams, TextDocumentIdentifier, Url};
use rstest::{fixture, rstest};
use rstest_bdd_server::discovery::WorkspaceInfo;
use std::path::Path;
use tempfile::TempDir;


//! Behavioural test for `.feature` file indexing on save.
};

#[fixture]
fn workspace_root() -> TempDir {
    let Ok(workspace_root) = TempDir::new() else {
        panic!("create workspace root");
    };
    workspace_root
}

#[fixture]
fn state_for_workspace(#[default(Path::new(""))] root: &Path) -> ServerState {
    let mut state = ServerState::new(ServerConfig::default());
    if let Err(error) = state.set_workspace_info(WorkspaceInfo {
        root: root.to_path_buf(),
        packages: Vec::new(),
    }) {
        panic!("configure workspace root: {error}");
    }
    state
}

#[rstest]
fn did_save_indexes_feature_files_and_caches_result(
    workspace_root: TempDir,
    #[with(workspace_root.path())] mut state_for_workspace: ServerState,
) {
    let path = workspace_root.path().join("demo.feature");
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

    handle_did_save_text_document(&mut state_for_workspace, params);

    let index = state_for_workspace
        .feature_index(&path)
        .expect("feature index cached");
    assert_eq!(index.steps.len(), 1);
    let step = index.steps.first().expect("expected indexed step");
    assert!(step.docstring.is_some());
}

#[rstest]
fn did_save_normalizes_disk_feature_source(
    workspace_root: TempDir,
    #[with(workspace_root.path())] mut state_for_workspace: ServerState,
) {
    let path = workspace_root.path().join("missing-newline.feature");
    std::fs::write(&path, "Feature: demo\n  Scenario: s\n    Given a message")
        .expect("write feature file");
    let uri = Url::from_file_path(&path).expect("file URI");
    let params = DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri },
        text: None,
    };

    handle_did_save_text_document(&mut state_for_workspace, params);

    let index = state_for_workspace
        .feature_index(&path)
        .expect("feature index cached");
    assert!(index.source.ends_with('\n'));
}

#[rstest]
fn did_save_rejects_feature_files_outside_workspace_root(
    workspace_root: TempDir,
    #[with(workspace_root.path())] mut state_for_workspace: ServerState,
) {
    assert!(
        workspace_root.path().is_dir(),
        "workspace root fixture should create a directory"
    );
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

    handle_did_save_text_document(&mut state_for_workspace, params);

    assert!(state_for_workspace.feature_index(&path).is_none());
}

#[rstest]
fn did_save_with_text_indexes_paths_outside_workspace_root(
    workspace_root: TempDir,
    #[with(workspace_root.path())] mut state_for_workspace: ServerState,
) {
    assert!(
        workspace_root.path().is_dir(),
        "workspace root fixture should create a directory"
    );
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

    handle_did_save_text_document(&mut state_for_workspace, params);

    assert!(state_for_workspace.feature_index(&path).is_some());
}
