//! Behavioural test for `.feature` file indexing on save.

use lsp_types::{DidSaveTextDocumentParams, TextDocumentIdentifier, Url};
use rstest_bdd_server::config::ServerConfig;
use rstest_bdd_server::handlers::handle_did_save_text_document;
use rstest_bdd_server::server::ServerState;
use tempfile::TempDir;

#[expect(clippy::expect_used, reason = "behavioural tests use explicit panics")]
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

    let mut state = ServerState::new(ServerConfig::default());
    handle_did_save_text_document(&mut state, params);

    let index = state.feature_index(&path).expect("feature index cached");
    assert_eq!(index.steps.len(), 1);
    let step = index.steps.first().expect("expected indexed step");
    assert!(step.docstring.is_some());
}
