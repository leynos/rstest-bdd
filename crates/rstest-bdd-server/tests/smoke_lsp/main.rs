//! End-to-end smoke tests for the `rstest-bdd-lsp` binary.
//!
//! These tests start the language server as a child process, send JSON-RPC
//! messages over stdin/stdout, and verify correct responses and diagnostics.
//! They validate the full stack: CLI argument parsing, server startup,
//! JSON-RPC communication, indexing pipeline, handler responses, and
//! graceful shutdown.

mod wire;

use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin};

use rstest::{fixture, rstest};
use serde_json::{Value, json};
use tempfile::TempDir;

use wire::{
    MessageReceiver, did_save, initialize, is_non_empty_diagnostics, shutdown_and_exit,
    spawn_server,
};

/// Maximum number of JSON-RPC messages to scan through when waiting for an
/// expected response or notification.  Extracted as a constant so CI can tune
/// the value in one place if interleaved traffic grows.
const MAX_RECV_MESSAGES: usize = 20;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Fixture providing a temporary directory for each test.
#[fixture]
fn temp_dir() -> TempDir {
    new_temp_dir()
}

/// Fixture providing an initialized LSP server backed by a temporary
/// directory.  The server is spawned, the initialize handshake is
/// performed, and the caller receives handles needed for interaction
/// and teardown.
#[fixture]
fn server(temp_dir: TempDir) -> ServerHandle {
    init_server_handle(temp_dir)
}

/// Create a new temporary directory, panicking with a descriptive
/// message on failure.
#[expect(
    clippy::expect_used,
    reason = "temp dir creation failure is a test-fatal I/O error"
)]
fn new_temp_dir() -> TempDir {
    TempDir::new().expect("temp dir")
}

/// Perform the full server setup: spawn, initialize handshake, and
/// return a [`ServerHandle`].
#[expect(
    clippy::expect_used,
    reason = "server setup failures are test-fatal conditions"
)]
fn init_server_handle(temp_dir: TempDir) -> ServerHandle {
    let root_uri = lsp_types::Url::from_directory_path(temp_dir.path()).expect("dir URI");

    let mut child = spawn_server(&[]);
    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let receiver = MessageReceiver::spawn(BufReader::new(stdout));

    let init_response = initialize(&mut stdin, &receiver, root_uri.as_str());

    ServerHandle {
        dir: temp_dir,
        child,
        stdin,
        receiver,
        init_response,
    }
}

/// Holds the state of a running LSP server for the duration of a test.
struct ServerHandle {
    /// Temporary directory whose lifetime pins the server's workspace.
    dir: TempDir,
    child: Child,
    stdin: ChildStdin,
    receiver: MessageReceiver,
    /// The response from the `initialize` request.
    init_response: Value,
}

impl ServerHandle {
    /// Convenience accessor for the workspace root path.
    fn workspace_root(&self) -> &Path {
        self.dir.path()
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// A feature file and its matching Rust step file, created inside a
/// temporary workspace directory.
struct TestFiles {
    feature_path: PathBuf,
    rust_path: PathBuf,
    rust_uri: lsp_types::Url,
}

/// Create a minimal feature file and a matching Rust step file inside `dir`.
#[expect(
    clippy::expect_used,
    reason = "file-write failures are test-fatal I/O errors"
)]
fn create_test_files(dir: &Path) -> TestFiles {
    let feature_path = dir.join("test.feature");
    std::fs::write(
        &feature_path,
        concat!(
            "Feature: smoke\n",
            "  Scenario: basic\n",
            "    Given a user exists\n",
        ),
    )
    .expect("write feature");

    let rust_path = dir.join("steps.rs");
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

    let rust_uri = lsp_types::Url::from_file_path(&rust_path).expect("rust URI");

    TestFiles {
        feature_path,
        rust_path,
        rust_uri,
    }
}

/// Send `didSave` for both files and wait for a `publishDiagnostics`
/// notification confirming that indexing has completed.
#[expect(
    clippy::expect_used,
    reason = "missing diagnostics notification is a test-fatal condition"
)]
fn index_and_wait(stdin: &mut ChildStdin, receiver: &MessageReceiver, files: &TestFiles) {
    did_save(stdin, &files.feature_path);
    did_save(stdin, &files.rust_path);

    let expected_uri = files.rust_uri.as_str();
    receiver
        .recv_notification_matching(
            |msg| {
                msg.get("method").and_then(|m| m.as_str())
                    == Some("textDocument/publishDiagnostics")
                    && msg
                        .get("params")
                        .and_then(|p| p.get("uri"))
                        .and_then(|u| u.as_str())
                        == Some(expected_uri)
            },
            MAX_RECV_MESSAGES,
        )
        .expect("expected a publishDiagnostics notification after indexing");
}

/// Assert that `def_response` contains a non-empty array of locations
/// whose first entry points to a feature file.
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "response validation uses .expect() and indexing for clarity"
)]
fn validate_definition_locations(def_response: &Value) {
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

    let loc_uri = locations[0]["uri"].as_str().expect("location uri");
    assert!(
        loc_uri.ends_with("test.feature"),
        "location should point to feature file, got: {loc_uri}"
    );
}

// ---------------------------------------------------------------------------
// Smoke tests
// ---------------------------------------------------------------------------

#[rstest]
#[expect(
    clippy::indexing_slicing,
    reason = "test assertions use JSON indexing for clear failure messages"
)]
fn smoke_initialize_and_shutdown(mut server: ServerHandle) {
    // Verify server capabilities
    let caps = &server.init_response["result"]["capabilities"];
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
    let info = &server.init_response["result"]["serverInfo"];
    assert_eq!(info["name"], "rstest-bdd-lsp");

    shutdown_and_exit(&mut server.stdin, &server.receiver, &mut server.child, 99);
}

#[rstest]
#[expect(
    clippy::indexing_slicing,
    reason = "test assertions use JSON indexing for clear failure messages"
)]
fn smoke_definition_request_returns_locations(mut server: ServerHandle) {
    let files = create_test_files(server.workspace_root());
    index_and_wait(&mut server.stdin, &server.receiver, &files);

    // Send definition request for the Rust step function
    // (line 3, 0-indexed).
    let def_request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "textDocument/definition",
        "params": {
            "textDocument": { "uri": files.rust_uri.as_str() },
            "position": { "line": 3, "character": 0 }
        }
    });
    wire::send(&mut server.stdin, &def_request);

    let (def_response, _) = server.receiver.recv_response_for_id(2, MAX_RECV_MESSAGES);
    assert_eq!(def_response["id"], 2, "definition response id");
    validate_definition_locations(&def_response);

    shutdown_and_exit(&mut server.stdin, &server.receiver, &mut server.child, 99);
}

#[rstest]
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test assertions use .expect() and indexing for clear failure messages"
)]
fn smoke_diagnostics_published_for_unimplemented_step(mut server: ServerHandle) {
    // Write a feature file with a step that has no Rust implementation.
    let feature_path = server.workspace_root().join("unimpl.feature");
    std::fs::write(
        &feature_path,
        concat!(
            "Feature: unimplemented\n",
            "  Scenario: missing step\n",
            "    Given a step with no implementation\n",
        ),
    )
    .expect("write feature");

    // Index the feature file — this should trigger diagnostics.
    did_save(&mut server.stdin, &feature_path);

    // Read messages until we find a publishDiagnostics notification with
    // non-empty diagnostics.
    let diag_msg = server
        .receiver
        .recv_notification_matching(is_non_empty_diagnostics, MAX_RECV_MESSAGES)
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

    shutdown_and_exit(&mut server.stdin, &server.receiver, &mut server.child, 99);
}
