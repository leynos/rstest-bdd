//! End-to-end smoke tests for the `rstest-bdd-lsp` binary.
//!
//! These tests start the language server as a child process, send JSON-RPC
//! messages over stdin/stdout, and verify correct responses and diagnostics.
//! They validate the full stack: CLI argument parsing, server startup,
//! JSON-RPC communication, indexing pipeline, handler responses, and
//! graceful shutdown.

mod wire;

use std::io::BufReader;

use serde_json::json;
use tempfile::TempDir;

use wire::{
    MessageReceiver, did_save, initialize, is_non_empty_diagnostics, shutdown_and_exit,
    spawn_server,
};

#[test]
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test assertions use .expect() and indexing for clear failure messages"
)]
fn smoke_initialize_and_shutdown() {
    let dir = TempDir::new().expect("temp dir");
    let root_uri = lsp_types::Url::from_directory_path(dir.path()).expect("dir URI");

    let mut child = spawn_server(&[]);
    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let receiver = MessageReceiver::spawn(BufReader::new(stdout));

    let response = initialize(&mut stdin, &receiver, root_uri.as_str());

    // Verify server capabilities
    let caps = &response["result"]["capabilities"];
    assert!(
        caps["definitionProvider"].as_bool().unwrap_or(false)
            || caps["definitionProvider"].is_object(),
        "expected definitionProvider capability"
    );
    assert!(
        caps["implementationProvider"].as_bool().unwrap_or(false)
            || caps["implementationProvider"].is_object(),
        "expected implementationProvider capability"
    );

    // Verify server info
    let info = &response["result"]["serverInfo"];
    assert_eq!(info["name"], "rstest-bdd-lsp");

    shutdown_and_exit(&mut stdin, &receiver, &mut child, 99);
}

#[test]
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test assertions use .expect() and indexing for clear failure messages"
)]
fn smoke_definition_request_returns_locations() {
    let dir = TempDir::new().expect("temp dir");
    let root_uri = lsp_types::Url::from_directory_path(dir.path()).expect("dir URI");

    // Write a feature file and a Rust step definitions file.
    let feature_path = dir.path().join("test.feature");
    std::fs::write(
        &feature_path,
        concat!(
            "Feature: smoke\n",
            "  Scenario: basic\n",
            "    Given a user exists\n",
        ),
    )
    .expect("write feature");

    let rust_path = dir.path().join("steps.rs");
    std::fs::write(
        &rust_path,
        concat!(
            "use rstest_bdd_macros::given;\n",
            "\n",
            "#[given(\"a user exists\")]\n",
            "fn a_user_exists() {}\n",
        ),
    )
    .expect("write rust steps");

    let mut child = spawn_server(&[]);
    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let receiver = MessageReceiver::spawn(BufReader::new(stdout));

    initialize(&mut stdin, &receiver, root_uri.as_str());

    // Index both files via didSave.
    did_save(&mut stdin, &feature_path);
    did_save(&mut stdin, &rust_path);

    // Wait for the server to finish indexing by consuming diagnostics
    // notifications rather than relying on a wall-clock sleep.
    receiver
        .recv_notification_matching(
            |msg| {
                msg.get("method").and_then(|m| m.as_str())
                    == Some("textDocument/publishDiagnostics")
            },
            20,
        )
        .expect("expected a publishDiagnostics notification after indexing");

    // Send definition request for the Rust step function
    // (line 3, 0-indexed).
    let rust_uri = lsp_types::Url::from_file_path(&rust_path).expect("rust URI");
    let def_request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "textDocument/definition",
        "params": {
            "textDocument": { "uri": rust_uri.as_str() },
            "position": { "line": 3, "character": 0 }
        }
    });
    wire::send(&mut stdin, &def_request);

    let (def_response, _) = receiver.recv_response_for_id(2, 20);
    assert_eq!(def_response["id"], 2, "definition response id");

    // The result should contain at least one location.
    let result = &def_response["result"];
    assert!(
        result.is_array(),
        "expected array of locations, got: {result}"
    );

    let locations = result.as_array().expect("locations array");
    assert!(
        !locations.is_empty(),
        "expected at least one definition location"
    );

    // Verify the location points to the feature file.
    let first_loc = &locations[0];
    let loc_uri = first_loc["uri"].as_str().expect("location uri");
    assert!(
        loc_uri.ends_with("test.feature"),
        "location should point to feature file, got: {loc_uri}"
    );

    shutdown_and_exit(&mut stdin, &receiver, &mut child, 99);
}

#[test]
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test assertions use .expect() and indexing for clear failure messages"
)]
fn smoke_diagnostics_published_for_unimplemented_step() {
    let dir = TempDir::new().expect("temp dir");
    let root_uri = lsp_types::Url::from_directory_path(dir.path()).expect("dir URI");

    // Write a feature file with a step that has no Rust implementation.
    let feature_path = dir.path().join("unimpl.feature");
    std::fs::write(
        &feature_path,
        concat!(
            "Feature: unimplemented\n",
            "  Scenario: missing step\n",
            "    Given a step with no implementation\n",
        ),
    )
    .expect("write feature");

    let mut child = spawn_server(&[]);
    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let receiver = MessageReceiver::spawn(BufReader::new(stdout));

    initialize(&mut stdin, &receiver, root_uri.as_str());

    // Index the feature file — this should trigger diagnostics.
    did_save(&mut stdin, &feature_path);

    // Read messages until we find a publishDiagnostics notification with
    // non-empty diagnostics.
    let diag_msg = receiver
        .recv_notification_matching(is_non_empty_diagnostics, 20)
        .expect(
            "expected a publishDiagnostics notification \
             for the unimplemented step",
        );

    let diags = diag_msg["params"]["diagnostics"]
        .as_array()
        .expect("diagnostics array");
    assert!(
        !diags.is_empty(),
        "expected at least one diagnostic for unimplemented step"
    );

    // Check that the diagnostic message mentions the unimplemented step.
    let first_msg = diags[0]["message"].as_str().unwrap_or("");
    assert!(
        first_msg.contains("a step with no implementation")
            || first_msg.contains("No Rust implementation"),
        "diagnostic message should mention the unimplemented step, \
         got: {first_msg}"
    );

    shutdown_and_exit(&mut stdin, &receiver, &mut child, 99);
}
