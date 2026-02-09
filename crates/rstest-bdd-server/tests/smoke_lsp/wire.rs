//! JSON-RPC wire protocol helpers for smoke tests.
//!
//! Provides message encoding, a timeout-aware [`MessageReceiver`], and
//! server lifecycle helpers for driving the `rstest-bdd-lsp` binary in
//! end-to-end tests.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{Value, json};

/// Maximum time to wait for a single JSON-RPC message before assuming the
/// server has stalled.
const READ_TIMEOUT: Duration = Duration::from_secs(10);

/// Maximum time to wait for the server process to exit after the `exit`
/// notification is sent.
const EXIT_TIMEOUT: Duration = Duration::from_secs(5);

/// Maximum number of JSON-RPC messages to scan through during lifecycle
/// handshake exchanges (initialize, shutdown).
const MAX_HANDSHAKE_MESSAGES: usize = 10;

// ---------------------------------------------------------------------------
// Encoding
// ---------------------------------------------------------------------------

/// Encode a JSON-RPC message with Content-Length header.
#[expect(
    clippy::expect_used,
    reason = "serialization failure is a test-fatal programming error"
)]
pub fn encode_message(body: &Value) -> Vec<u8> {
    let body_bytes = serde_json::to_vec(body).expect("serialize JSON-RPC body");
    let mut msg = format!("Content-Length: {}\r\n\r\n", body_bytes.len()).into_bytes();
    msg.extend_from_slice(&body_bytes);
    msg
}

// ---------------------------------------------------------------------------
// MessageReceiver
// ---------------------------------------------------------------------------

/// A channel-based message receiver that reads JSON-RPC messages on a
/// background thread and delivers them with a configurable timeout.
///
/// This prevents the test suite from hanging indefinitely when the server
/// crashes or fails to produce output.
pub struct MessageReceiver {
    rx: mpsc::Receiver<Value>,
}

/// Read LSP headers and return the content length, or `None` on EOF or
/// when no body follows.
#[expect(
    clippy::expect_used,
    reason = "header parse failures are test-fatal programming errors"
)]
fn read_headers(reader: &mut BufReader<impl Read>) -> Option<usize> {
    let mut content_length: usize = 0;
    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).expect("read header line");
        if bytes_read == 0 {
            return None; // EOF
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break; // End of headers
        }
        if let Some(len_str) = trimmed.strip_prefix("Content-Length: ") {
            content_length = len_str.parse().expect("parse content length");
        }
    }
    if content_length == 0 {
        None
    } else {
        Some(content_length)
    }
}

impl MessageReceiver {
    /// Spawn a background reader thread for the given `BufReader`.
    ///
    /// Messages are decoded in the background and sent through an unbounded
    /// channel. The thread terminates when the reader reaches EOF.
    #[expect(
        clippy::expect_used,
        reason = "body parse failures are test-fatal programming errors"
    )]
    pub fn spawn(mut reader: BufReader<impl Read + Send + 'static>) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            while let Some(content_length) = read_headers(&mut reader) {
                let mut buf = vec![0u8; content_length];
                reader.read_exact(&mut buf).expect("read body");
                let msg: Value = serde_json::from_slice(&buf).expect("parse JSON body");
                if tx.send(msg).is_err() {
                    break;
                }
            }
        });
        Self { rx }
    }

    /// Receive the next message, blocking up to [`READ_TIMEOUT`].
    ///
    /// # Panics
    ///
    /// Panics if no message arrives within the timeout.
    #[expect(
        clippy::expect_used,
        reason = "timeout is a test-fatal condition indicating a server stall"
    )]
    pub fn recv(&self) -> Value {
        self.rx
            .recv_timeout(READ_TIMEOUT)
            .expect("timed out waiting for JSON-RPC message from server")
    }

    /// Receive messages until one has the given response `id`.
    ///
    /// Notifications (messages without an `id`) are collected and returned
    /// alongside the response so callers can inspect them if needed.
    pub fn recv_response_for_id(&self, id: u64, max_messages: usize) -> (Value, Vec<Value>) {
        let mut notifications = Vec::new();
        for _ in 0..max_messages {
            let msg = self.recv();
            if msg.get("id").and_then(Value::as_u64) == Some(id) {
                return (msg, notifications);
            }
            notifications.push(msg);
        }
        panic!("did not receive response with id {id} within {max_messages} messages");
    }

    /// Receive messages until one matches the predicate, or the limit is
    /// reached.
    pub fn recv_notification_matching(
        &self,
        predicate: impl Fn(&Value) -> bool,
        max_messages: usize,
    ) -> Option<Value> {
        for _ in 0..max_messages {
            let msg = self.recv();
            if predicate(&msg) {
                return Some(msg);
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Server lifecycle helpers
// ---------------------------------------------------------------------------

/// Spawn the `rstest-bdd-lsp` binary with the given extra arguments.
#[expect(
    clippy::expect_used,
    reason = "binary spawn failure is a test-fatal environment error"
)]
pub fn spawn_server(extra_args: &[&str]) -> Child {
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
#[expect(
    clippy::expect_used,
    reason = "write failure is a test-fatal I/O error"
)]
pub fn send(stdin: &mut impl Write, body: &Value) {
    stdin
        .write_all(&encode_message(body))
        .expect("write message");
    stdin.flush().expect("flush stdin");
}

/// Perform the initialize handshake and return the initialize result.
pub fn initialize(stdin: &mut impl Write, receiver: &MessageReceiver, root_uri: &str) -> Value {
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
    let (response, _notifications) = receiver.recv_response_for_id(1, MAX_HANDSHAKE_MESSAGES);

    let initialized_notification = json!({
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": {}
    });
    send(stdin, &initialized_notification);

    response
}

/// Send shutdown request and exit notification, then wait for exit.
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "shutdown/exit failures are test-fatal protocol errors"
)]
pub fn shutdown_and_exit(
    stdin: &mut impl Write,
    receiver: &MessageReceiver,
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
    let (shutdown_response, _) = receiver.recv_response_for_id(request_id, MAX_HANDSHAKE_MESSAGES);
    assert_eq!(shutdown_response["id"], request_id, "shutdown response id");

    let exit_notification = json!({
        "jsonrpc": "2.0",
        "method": "exit",
        "params": null
    });
    send(stdin, &exit_notification);

    // Wait for exit with a bounded timeout, killing the process if it
    // stalls to prevent CI hangs.
    let deadline = std::time::Instant::now() + EXIT_TIMEOUT;
    loop {
        match child.try_wait().expect("check server exit status") {
            Some(status) => {
                assert!(
                    status.success(),
                    "server should exit cleanly, got: {status}"
                );
                return;
            }
            None if std::time::Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "server did not exit within {} s; killed",
                    EXIT_TIMEOUT.as_secs()
                );
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// Send a `textDocument/didSave` notification for the given file.
#[expect(
    clippy::expect_used,
    reason = "file URI construction failure is a test-fatal path error"
)]
pub fn did_save(stdin: &mut impl Write, file_path: &Path) {
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

/// Return `true` when `msg` is a `publishDiagnostics` notification
/// carrying a non-empty diagnostics array.
#[expect(
    clippy::indexing_slicing,
    reason = "JSON path indexing returns Null for missing keys, which is safe"
)]
pub fn is_non_empty_diagnostics(msg: &Value) -> bool {
    msg.get("method").and_then(|m| m.as_str()) == Some("textDocument/publishDiagnostics")
        && msg["params"]["diagnostics"]
            .as_array()
            .is_some_and(|d| !d.is_empty())
}
