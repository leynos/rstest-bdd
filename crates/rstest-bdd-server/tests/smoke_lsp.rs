//! End-to-end smoke tests for the `rstest-bdd-lsp` binary.
//!
//! These tests start the language server as a child process, send JSON-RPC
//! messages over stdin/stdout, and verify correct responses and diagnostics.
//! They validate the full stack: CLI argument parsing, server startup,
//! JSON-RPC communication, indexing pipeline, handler responses, and
//! graceful shutdown.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "smoke tests use explicit panics and indexing for clarity"
)]

use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// JSON-RPC wire helpers
// ---------------------------------------------------------------------------

/// Encode a JSON-RPC message with Content-Length header.
fn encode_message(body: &Value) -> Vec<u8> {
    let body_bytes = serde_json::to_vec(body).expect("serialise JSON-RPC body");
    let mut msg = format!("Content-Length: {}\r\n\r\n", body_bytes.len()).into_bytes();
    msg.extend_from_slice(&body_bytes);
    msg
}

/// Read a single JSON-RPC message from the given reader.
///
/// Blocks until a complete message is available or the reader is exhausted.
///
/// # Panics
///
/// Panics if the Content-Length header is missing or the body cannot be
/// parsed as JSON.
fn read_message(reader: &mut BufReader<impl Read>) -> Value {
    let mut content_length: usize = 0;
    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).expect("read header line");
        assert!(
            bytes_read > 0,
            "unexpected end of stream while reading headers"
        );
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(len_str) = trimmed.strip_prefix("Content-Length: ") {
            content_length = len_str.parse().expect("parse content length");
        }
    }
    assert!(content_length > 0, "missing Content-Length header");
    let mut buf = vec![0u8; content_length];
    reader.read_exact(&mut buf).expect("read body");
    serde_json::from_slice(&buf).expect("parse JSON body")
}

/// Read JSON-RPC messages until finding a response with the given `id`.
///
/// Notifications (messages without an `id`) are collected and returned
/// alongside the response so callers can inspect them if needed. Panics
/// if the response is not received within `max_messages` reads.
fn read_response_for_id(
    reader: &mut BufReader<impl Read>,
    id: u64,
    max_messages: usize,
) -> (Value, Vec<Value>) {
    let mut notifications = Vec::new();
    for _ in 0..max_messages {
        let msg = read_message(reader);
        if msg.get("id").and_then(Value::as_u64) == Some(id) {
            return (msg, notifications);
        }
        notifications.push(msg);
    }
    panic!("did not receive response with id {id} within {max_messages} messages");
}

/// Read JSON-RPC notifications until a message matching the predicate
/// arrives, or the attempt limit is reached.
fn read_notification_matching(
    reader: &mut BufReader<impl Read>,
    predicate: impl Fn(&Value) -> bool,
    max_messages: usize,
) -> Option<Value> {
    for _ in 0..max_messages {
        let msg = read_message(reader);
        if predicate(&msg) {
            return Some(msg);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Server lifecycle helpers
// ---------------------------------------------------------------------------

/// Spawn the `rstest-bdd-lsp` binary with the given extra arguments.
fn spawn_server(extra_args: &[&str]) -> Child {
    let binary = env!("CARGO_BIN_EXE_rstest-bdd-lsp");
    Command::new(binary)
        .args(["--log-level", "error", "--debounce-ms", "0"])
        .args(extra_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start rstest-bdd-lsp binary")
}

/// Send a JSON-RPC message to the server's stdin.
fn send(stdin: &mut impl Write, body: &Value) {
    stdin
        .write_all(&encode_message(body))
        .expect("write message");
    stdin.flush().expect("flush stdin");
}

/// Perform the initialize handshake and return the initialize result.
fn initialise(stdin: &mut impl Write, reader: &mut BufReader<impl Read>, root_uri: &str) -> Value {
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "processId": null,
            "capabilities": {},
            "rootUri": root_uri,
        }
    });
    send(stdin, &init_request);
    let (response, _notifications) = read_response_for_id(reader, 1, 10);

    let initialized_notification = json!({
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": {}
    });
    send(stdin, &initialized_notification);

    response
}

/// Send shutdown request and exit notification, then wait for exit.
fn shutdown_and_exit(
    stdin: &mut impl Write,
    reader: &mut BufReader<impl Read>,
    child: &mut Child,
    request_id: u64,
) {
    let shutdown_request = json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "shutdown",
        "params": null
    });
    send(stdin, &shutdown_request);
    let (shutdown_response, _) = read_response_for_id(reader, request_id, 10);
    assert_eq!(shutdown_response["id"], request_id, "shutdown response id");

    let exit_notification = json!({
        "jsonrpc": "2.0",
        "method": "exit",
        "params": null
    });
    send(stdin, &exit_notification);

    let status = child.wait().expect("wait for server exit");
    assert!(
        status.success(),
        "server should exit cleanly, got: {status}"
    );
}

/// Send a `textDocument/didSave` notification for the given file.
fn did_save(stdin: &mut impl Write, file_path: &Path) {
    let uri = lsp_types::Url::from_file_path(file_path).expect("file URI");
    let notification = json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didSave",
        "params": {
            "textDocument": { "uri": uri.as_str() }
        }
    });
    send(stdin, &notification);
}

// ---------------------------------------------------------------------------
// Smoke tests
// ---------------------------------------------------------------------------

#[test]
fn smoke_initialise_and_shutdown() {
    let dir = TempDir::new().expect("temp dir");
    let root_uri = lsp_types::Url::from_directory_path(dir.path()).expect("dir URI");

    let mut child = spawn_server(&[]);
    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);

    let response = initialise(&mut stdin, &mut reader, root_uri.as_str());

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

    shutdown_and_exit(&mut stdin, &mut reader, &mut child, 99);
}

#[test]
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
    let mut reader = BufReader::new(stdout);

    initialise(&mut stdin, &mut reader, root_uri.as_str());

    // Index both files via didSave.
    did_save(&mut stdin, &feature_path);
    did_save(&mut stdin, &rust_path);

    // Allow a short pause for indexing to complete.
    std::thread::sleep(Duration::from_millis(100));

    // Send definition request for the Rust step function (line 3, 0-indexed).
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
    send(&mut stdin, &def_request);

    let (def_response, _) = read_response_for_id(&mut reader, 2, 20);
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

    shutdown_and_exit(&mut stdin, &mut reader, &mut child, 99);
}

#[test]
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
    let mut reader = BufReader::new(stdout);

    initialise(&mut stdin, &mut reader, root_uri.as_str());

    // Index the feature file — this should trigger diagnostics.
    did_save(&mut stdin, &feature_path);

    // Read messages until we find a publishDiagnostics notification with
    // non-empty diagnostics.
    let diag_msg = read_notification_matching(
        &mut reader,
        |msg| {
            msg.get("method").and_then(|m| m.as_str()) == Some("textDocument/publishDiagnostics")
                && msg["params"]["diagnostics"]
                    .as_array()
                    .is_some_and(|d| !d.is_empty())
        },
        20,
    );

    let diag_msg =
        diag_msg.expect("expected a publishDiagnostics notification for the unimplemented step");
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
        "diagnostic message should mention the unimplemented step, got: {first_msg}"
    );

    shutdown_and_exit(&mut stdin, &mut reader, &mut child, 99);
}
