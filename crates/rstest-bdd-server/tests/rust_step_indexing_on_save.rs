//! Behavioural test for Rust step indexing on save.

use std::path::Path;

use lsp_types::{
    DidSaveTextDocumentParams, NumberOrString, PublishDiagnosticsParams, TextDocumentIdentifier,
    Url,
};
use rstest_bdd_server::config::ServerConfig;
use rstest_bdd_server::handlers::handle_did_save_text_document;
use rstest_bdd_server::server::ServerState;
use tempfile::TempDir;

fn did_save_params(uri: Url, text: Option<&str>) -> DidSaveTextDocumentParams {
    DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri },
        text: text.map(str::to_owned),
    }
}

/// Decode every complete Content-Length-framed message in the captured LSP
/// transport output.
fn decode_lsp_messages(mut bytes: &[u8]) -> Vec<serde_json::Value> {
    let mut messages = Vec::new();
    while !bytes.is_empty() {
        let (message, remaining) = decode_lsp_message(bytes);
        messages.push(message);
        bytes = remaining;
    }
    messages
}

/// Decode one Content-Length-framed message from the start of `bytes`.
///
/// Returns the decoded JSON-RPC value alongside the unconsumed transport
/// bytes that follow the decoded frame.
fn decode_lsp_message(bytes: &[u8]) -> (serde_json::Value, &[u8]) {
    let (body, remaining) = split_lsp_frame(bytes);
    let Ok(message) = serde_json::from_slice(body) else {
        panic!("LSP body must contain valid JSON-RPC");
    };
    (message, remaining)
}

/// Split one Content-Length frame into its JSON body and the bytes following
/// the frame.
///
/// Validates the `Content-Length: ` prefix, locates the complete `\r\n\r\n`
/// header separator, parses the frame length, and returns the body slice
/// alongside the unconsumed remainder.
#[expect(
    clippy::indexing_slicing,
    reason = "slices are bounds-safe after the preceding bounds checks"
)]
fn split_lsp_frame(bytes: &[u8]) -> (&[u8], &[u8]) {
    const HEADER: &[u8] = b"Content-Length: ";
    const HEADER_SEPARATOR: &[u8] = b"\r\n\r\n";

    let Some(header) = bytes.strip_prefix(HEADER) else {
        panic!("expected Content-Length header in LSP transport output");
    };
    let Some(header_end) = header
        .windows(HEADER_SEPARATOR.len())
        .position(|window| window == HEADER_SEPARATOR)
    else {
        panic!("expected complete LSP Content-Length header");
    };
    let length = parse_content_length(&header[..header_end]);
    // Bounds-safe: `header_end` is the start of a complete separator window,
    // so it and the separator always fit within `header`.
    let body_and_remaining = &header[header_end + HEADER_SEPARATOR.len()..];
    let Some(body) = body_and_remaining.get(..length) else {
        panic!("LSP body must match its Content-Length header");
    };
    // Bounds-safe: `body` succeeding above proves `length` fits in
    // `body_and_remaining`, so the trailing slice is always valid.
    let remaining = &body_and_remaining[length..];
    (body, remaining)
}

/// Parse the numeric `Content-Length` header value from its octets.
fn parse_content_length(length_bytes: &[u8]) -> usize {
    let Some(length_text) = std::str::from_utf8(length_bytes).ok() else {
        panic!("Content-Length header must be valid UTF-8");
    };
    let Some(length) = length_text.parse::<usize>().ok() else {
        panic!("Content-Length header must be numeric");
    };
    length
}

/// Feature source exercised by the recoverable-diagnostic scenario.
const FEATURE_SOURCE: &str = concat!(
    "Feature: demo\n",
    "  Scenario: example\n",
    "    Given a valid step\n",
);

/// Rust source with duplicate step attributes on one function.
const MALFORMED_SOURCE: &str = concat!(
    "use rstest_bdd_macros::{given, when};\n",
    "\n",
    "#[given(\"duplicate\")]\n",
    "#[when(\"conflict\")]\n",
    "fn conflicting_step() {}\n",
    "\n",
    "#[given(\"a valid step\")]\n",
    "fn valid_step() {}\n",
);

/// Rust source with a single valid step definition.
const CORRECTED_SOURCE: &str = concat!(
    "use rstest_bdd_macros::given;\n",
    "\n",
    "#[given(\"a valid step\")]\n",
    "fn valid_step() {}\n",
);

/// Rust source that fails to parse as a syntax tree.
const PARSE_FAILURE_SOURCE: &str = "fn incomplete(";

/// Write a Rust source file to disk and notify the server of the save.
#[expect(
    clippy::expect_used,
    reason = "test helper preserves explicit failure messages for filesystem writes"
)]
fn save_rust_source(
    state: &mut ServerState,
    path: &Path,
    uri: &Url,
    source: &str,
    failure_context: &'static str,
) {
    std::fs::write(path, source).expect(failure_context);
    handle_did_save_text_document(state, did_save_params(uri.clone(), None));
}

/// Extract `publishDiagnostics` notifications for one URI from captured LSP
/// transport output.
fn rust_diagnostics_for_uri(captured: &[u8], uri: &Url) -> Vec<PublishDiagnosticsParams> {
    decode_lsp_messages(captured)
        .into_iter()
        .filter(|message| {
            message.get("method").and_then(serde_json::Value::as_str)
                == Some("textDocument/publishDiagnostics")
        })
        .filter_map(|message| message.get("params").cloned())
        .filter_map(|params| serde_json::from_value(params).ok())
        .filter(|params: &PublishDiagnosticsParams| &params.uri == uri)
        .collect()
}

/// Return positions whose diagnostics carry the `multiple-step-attributes`
/// code.
fn recoverable_diagnostic_positions(diagnostics: &[PublishDiagnosticsParams]) -> Vec<usize> {
    diagnostics
        .iter()
        .enumerate()
        .filter_map(|(position, params)| {
            params
                .diagnostics
                .iter()
                .any(|diagnostic| {
                    diagnostic.code.as_ref().is_some_and(|code| {
                        matches!(
                            code,
                            NumberOrString::String(code) if code == "multiple-step-attributes"
                        )
                    })
                })
                .then_some(position)
        })
        .collect()
}

/// Assert that every recoverable diagnostic publication is immediately
/// followed by a clearing publication.
fn assert_recoverable_diagnostics_are_cleared(diagnostics: &[PublishDiagnosticsParams]) {
    let recoverable_positions = recoverable_diagnostic_positions(diagnostics);
    assert_eq!(
        recoverable_positions.len(),
        2,
        "both recoverable indexing saves should publish diagnostics"
    );
    for recoverable_position in recoverable_positions {
        assert!(
            diagnostics
                .get(recoverable_position + 1)
                .is_some_and(|params| params.diagnostics.is_empty()),
            "the clearing publication must immediately follow the recoverable diagnostic"
        );
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
async fn did_save_clears_recoverable_index_diagnostics_after_success_and_parse_failure() {
    use async_lsp::MainLoop;
    use async_lsp::router::Router;
    use tokio::io::AsyncReadExt;
    use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

    let dir = TempDir::new().expect("temp dir");
    let feature_path = dir.path().join("scenario.feature");
    let rust_path = dir.path().join("steps.rs");
    let feature_uri = Url::from_file_path(&feature_path).expect("feature URI");
    let rust_uri = Url::from_file_path(&rust_path).expect("Rust URI");

    let (mainloop, client) = MainLoop::new_server(|_client| Router::new(()));
    let mut state = ServerState::new(ServerConfig::default());
    handle_did_save_text_document(
        &mut state,
        did_save_params(feature_uri, Some(FEATURE_SOURCE)),
    );
    state.set_client(client);

    save_rust_source(
        &mut state,
        &rust_path,
        &rust_uri,
        MALFORMED_SOURCE,
        "write recoverable diagnostic source",
    );
    save_rust_source(
        &mut state,
        &rust_path,
        &rust_uri,
        CORRECTED_SOURCE,
        "write corrected source",
    );
    save_rust_source(
        &mut state,
        &rust_path,
        &rust_uri,
        MALFORMED_SOURCE,
        "rewrite recoverable diagnostic source",
    );
    save_rust_source(
        &mut state,
        &rust_path,
        &rust_uri,
        PARSE_FAILURE_SOURCE,
        "write parse failure source",
    );

    let (writer, mut reader) = tokio::io::duplex(64 * 1024);
    let _ = mainloop
        .run_buffered(tokio::io::empty().compat(), writer.compat_write())
        .await;

    let mut captured = Vec::new();
    assert!(
        reader.read_to_end(&mut captured).await.is_ok(),
        "read captured LSP notifications"
    );
    assert_recoverable_diagnostics_are_cleared(&rust_diagnostics_for_uri(&captured, &rust_uri));
}
