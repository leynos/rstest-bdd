//! Behavioural test for Rust step indexing on save.

use lsp_types::{DidSaveTextDocumentParams, TextDocumentIdentifier, Url};
use rstest_bdd_server::config::ServerConfig;
use rstest_bdd_server::handlers::handle_did_save_text_document;
use rstest_bdd_server::server::ServerState;
use tempfile::TempDir;

#[expect(clippy::expect_used, reason = "behavioural tests use explicit panics")]
#[test]
fn did_save_indexes_rust_step_files_and_caches_result() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("steps.rs");
    std::fs::write(
        &path,
        concat!(
            "use rstest_bdd_macros::{given, when};\n",
            "\n",
            "#[given(\"a message\")]\n",
            "fn a_message() {}\n",
            "\n",
            "#[when]\n",
            "fn I_do_the_thing() {}\n",
        ),
    )
    .expect("write rust source file");

    let uri = Url::from_file_path(&path).expect("file URI");
    let params = DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri },
        text: None,
    };

    let mut state = ServerState::new(ServerConfig::default());
    handle_did_save_text_document(&mut state, params);

    let index = state
        .rust_step_index(&path)
        .expect("rust step index cached");
    assert_eq!(index.step_definitions.len(), 2);
    let inferred = index
        .step_definitions
        .iter()
        .find(|step| step.function.name == "I_do_the_thing")
        .expect("expected inferred step");
    assert!(inferred.pattern_inferred);
    assert_eq!(inferred.pattern, "I do the thing");
}
