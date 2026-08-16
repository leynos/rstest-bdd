//! Behavioural test for Rust step indexing on save.

use lsp_types::{DidSaveTextDocumentParams, TextDocumentIdentifier, Url};
use rstest_bdd_server::config::ServerConfig;
use rstest_bdd_server::handlers::handle_did_save_text_document;
use rstest_bdd_server::server::ServerState;
use tempfile::TempDir;

fn did_save_params(uri: Url, text: &str) -> DidSaveTextDocumentParams {
    DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri },
        text: Some(text.to_owned()),
    }
}

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

#[test]
fn did_save_prefers_provided_text_over_filesystem_contents() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("steps.rs");
    std::fs::write(
        &path,
        concat!(
            "use rstest_bdd_macros::given;\n",
            "\n",
            "#[given(\"disk pattern\")]\n",
            "fn a_message() {}\n",
        ),
    )
    .expect("write rust source file");

    let provided_text = concat!(
        "use rstest_bdd_macros::{given, when};\n",
        "\n",
        "#[given(\"provided pattern\")]\n",
        "fn a_message() {}\n",
        "\n",
        "#[when]\n",
        "fn I_do_the_thing() {}\n",
    )
    .to_string();

    let uri = Url::from_file_path(&path).expect("file URI");
    let params = DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri },
        text: Some(provided_text),
    };

    let mut state = ServerState::new(ServerConfig::default());
    handle_did_save_text_document(&mut state, params);

    let index = state
        .rust_step_index(&path)
        .expect("rust step index cached");
    assert_eq!(index.step_definitions.len(), 2);
    let a_message = index
        .step_definitions
        .iter()
        .find(|step| step.function.name == "a_message")
        .expect("expected a_message step");
    assert_eq!(a_message.pattern, "provided pattern");
}

#[test]
fn did_save_retains_valid_steps_after_recoverable_attribute_diagnostic() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("steps.rs");
    let source = concat!(
        "use rstest_bdd_macros::{given, when};\n",
        "\n",
        "#[given(\"first\")]\n",
        "#[when(\"second\")]\n",
        "fn conflicting_step() {}\n",
        "\n",
        "#[given(\"valid\")]\n",
        "fn valid_step() {}\n",
    );

    let uri = Url::from_file_path(&path).expect("file URI");
    let params = DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri },
        text: Some(source.to_owned()),
    };

    let mut state = ServerState::new(ServerConfig::default());
    handle_did_save_text_document(&mut state, params);

    let index = state
        .rust_step_index(&path)
        .expect("valid definitions should remain cached");
    assert_eq!(index.step_definitions.len(), 1);
    let step = index
        .step_definitions
        .first()
        .expect("one valid definition");
    assert_eq!(step.function.name, "valid_step");
}

#[tokio::test]
async fn did_save_publishes_and_clears_recoverable_index_diagnostics() {
    use async_lsp::MainLoop;
    use async_lsp::router::Router;
    use tokio::io::AsyncReadExt;
    use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

    let dir = TempDir::new().expect("temp dir");
    let feature_path = dir.path().join("scenario.feature");
    let rust_path = dir.path().join("steps.rs");
    let feature_uri = Url::from_file_path(&feature_path).expect("feature URI");
    let rust_uri = Url::from_file_path(&rust_path).expect("Rust URI");
    let feature_source = concat!(
        "Feature: demo\n",
        "  Scenario: example\n",
        "    Given a valid step\n",
    );
    let malformed_source = concat!(
        "use rstest_bdd_macros::{given, when};\n",
        "\n",
        "#[given(\"duplicate\")]\n",
        "#[when(\"conflict\")]\n",
        "fn conflicting_step() {}\n",
        "\n",
        "#[given(\"a valid step\")]\n",
        "fn valid_step() {}\n",
    );
    let corrected_source = concat!(
        "use rstest_bdd_macros::given;\n",
        "\n",
        "#[given(\"a valid step\")]\n",
        "fn valid_step() {}\n",
    );

    let (mainloop, client) = MainLoop::new_server(|_client| Router::new(()));
    let mut state = ServerState::new(ServerConfig::default());
    handle_did_save_text_document(&mut state, did_save_params(feature_uri, feature_source));
    state.set_client(client);

    handle_did_save_text_document(
        &mut state,
        did_save_params(rust_uri.clone(), malformed_source),
    );
    handle_did_save_text_document(&mut state, did_save_params(rust_uri, corrected_source));

    let (writer, mut reader) = tokio::io::duplex(64 * 1024);
    let _ = mainloop
        .run_buffered(tokio::io::empty().compat(), writer.compat_write())
        .await;

    let mut captured = Vec::new();
    assert!(
        reader.read_to_end(&mut captured).await.is_ok(),
        "read captured LSP notifications"
    );
    let output = String::from_utf8(captured).expect("valid JSON-RPC output");
    let recoverable_position = output
        .find("multiple-step-attributes")
        .expect("recoverable indexing diagnostic should be published");
    let clearing_position = output[recoverable_position..]
        .find("\"diagnostics\":[]")
        .expect("corrected source should clear stale indexing diagnostics");
    assert!(
        clearing_position > 0,
        "the clearing publication must follow the recoverable diagnostic"
    );
}
