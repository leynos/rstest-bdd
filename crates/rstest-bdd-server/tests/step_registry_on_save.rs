//! Behavioural tests for compiled step registry updates on save.

use gherkin::StepType;
use lsp_types::{DidSaveTextDocumentParams, TextDocumentIdentifier, Url};
use rstest_bdd_server::config::ServerConfig;
use rstest_bdd_server::handlers::handle_did_save_text_document;
use rstest_bdd_server::server::ServerState;
use tempfile::TempDir;

#[expect(clippy::expect_used, reason = "behavioural tests use explicit panics")]
#[test]
fn did_save_compiles_step_patterns_and_updates_registry_incrementally() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("steps.rs");

    let first = concat!(
        "use rstest_bdd_macros::{given, when};\n",
        "\n",
        "#[given(\"I have {n:u32}\")]\n",
        "fn have_number() {}\n",
        "\n",
        "#[when(\"I add 1\")]\n",
        "fn add_one() {}\n",
    );
    std::fs::write(&path, first).expect("write initial rust source file");

    let uri = Url::from_file_path(&path).expect("file URI");
    let params = DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        text: None,
    };

    let mut state = ServerState::new(ServerConfig::default());
    handle_did_save_text_document(&mut state, params);

    let given = state.step_registry().steps_for_keyword(StepType::Given);
    assert_eq!(given.len(), 1);
    let matcher = given.first().expect("compiled given matcher");
    assert!(matcher.regex.is_match("I have 42"));

    let when = state.step_registry().steps_for_keyword(StepType::When);
    assert_eq!(when.len(), 1);

    let second = concat!(
        "use rstest_bdd_macros::when;\n",
        "\n",
        "#[when(\"I add 1\")]\n",
        "fn add_one() {}\n",
    );
    std::fs::write(&path, second).expect("write updated rust source file");

    let params = DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri },
        text: None,
    };
    handle_did_save_text_document(&mut state, params);

    assert_eq!(
        state
            .step_registry()
            .steps_for_keyword(StepType::Given)
            .len(),
        0
    );
    assert_eq!(
        state
            .step_registry()
            .steps_for_keyword(StepType::When)
            .len(),
        1
    );
    assert_eq!(state.step_registry().steps_for_file(&path).len(), 1);
}

#[expect(clippy::expect_used, reason = "behavioural tests use explicit panics")]
#[test]
fn did_save_skips_invalid_step_patterns_without_blocking_valid_steps() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("steps.rs");

    std::fs::write(
        &path,
        concat!(
            "use rstest_bdd_macros::{given, when};\n",
            "\n",
            "#[given(\"unclosed {\")]\n",
            "fn invalid_pattern() {}\n",
            "\n",
            "#[when(\"I add 1\")]\n",
            "fn add_one() {}\n",
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

    let rust_index = state
        .rust_step_index(&path)
        .expect("rust step index cached");
    assert_eq!(rust_index.step_definitions.len(), 2);

    assert_eq!(
        state
            .step_registry()
            .steps_for_keyword(StepType::Given)
            .len(),
        0
    );

    let when = state.step_registry().steps_for_keyword(StepType::When);
    assert_eq!(when.len(), 1);
    let matcher = when.first().expect("compiled when matcher");
    assert!(matcher.regex.is_match("I add 1"));
}
