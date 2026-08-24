//! End-to-end smoke tests for empty-vector clearing of published diagnostics.
//!
//! Split out of `main.rs` so each smoke-test file stays within the 400-line
//! limit. Reuses the `ServerHandle` fixture, `wire` helpers, and constants
//! from the crate root.

use rstest::rstest;

use super::{
    MAX_RECV_MESSAGES,
    ServerHandle,
    server,
    wire::{did_save, is_non_empty_diagnostics, shutdown_and_exit},
};

/// Exercise the canonical publication boundary end-to-end through the public
/// publishers. An unimplemented step first emits a non-empty
/// `publishDiagnostics` notification; once the matching Rust step is added, the
/// feature re-publishes an *empty* diagnostics array to clear the resolved
/// diagnostic. This pins that `publish_with` actually notifies the client and
/// that empty vectors are published (not suppressed) — behaviour the
/// payload-only `prepare_publish` tests cannot observe.
#[rstest]
#[expect(
    clippy::indexing_slicing,
    reason = "test assertions use .expect() and indexing for clear failure messages"
)]
fn smoke_feature_diagnostics_cleared_once_step_implemented(mut server: ServerHandle) {
    let dir = server.workspace_root().to_path_buf();
    let feature_path = dir.join("clearing.feature");
    std::fs::write(
        &feature_path,
        concat!(
            "Feature: clearing\n",
            "  Scenario: pending\n",
            "    Given a pending step\n",
        ),
    )
    .expect("write feature");
    let feature_uri = lsp_types::Url::from_file_path(&feature_path).expect("feature URI");
    let feature_uri = feature_uri.as_str().to_owned();

    // Saving the feature publishes a non-empty diagnostic for the
    // unimplemented step through the canonical boundary.
    did_save(&mut server.stdin, &feature_path);
    let non_empty_uri = feature_uri.clone();
    server
        .receiver
        .recv_notification_matching(
            move |msg| {
                is_non_empty_diagnostics(msg)
                    && msg["params"]["uri"].as_str() == Some(non_empty_uri.as_str())
            },
            MAX_RECV_MESSAGES,
        )
        .expect("expected non-empty diagnostics for the unimplemented step");

    // Implement the step in a Rust file; saving it re-publishes every feature,
    // and the now-satisfied feature must publish an empty diagnostics array to
    // clear the stale diagnostic.
    let rust_path = dir.join("clearing_steps.rs");
    std::fs::write(
        &rust_path,
        concat!(
            "use rstest_bdd_macros::given;\n",
            "\n",
            "#[given(\"a pending step\")]\n",
            "fn a_pending_step() {}\n",
        ),
    )
    .expect("write rust steps");

    did_save(&mut server.stdin, &rust_path);
    server
        .receiver
        .recv_notification_matching(
            move |msg| {
                msg.get("method").and_then(|m| m.as_str())
                    == Some("textDocument/publishDiagnostics")
                    && msg["params"]["uri"].as_str() == Some(feature_uri.as_str())
                    && msg["params"]["diagnostics"]
                        .as_array()
                        .is_some_and(Vec::is_empty)
            },
            MAX_RECV_MESSAGES,
        )
        .expect("expected empty diagnostics clearing the resolved feature");

    shutdown_and_exit(&mut server.stdin, &server.receiver, &mut server.child, 99);
}

/// Exercise the Rust side of the canonical boundary end-to-end: a Rust step
/// with no feature referencing it is reported as unused (non-empty
/// `publishDiagnostics`), and once a feature uses it, re-saving the Rust file
/// re-publishes an empty diagnostics array for the same Rust URI — proving
/// `publish_rust_index_result_diagnostics` clears stale diagnostics with an
/// empty vector, which the payload-only `prepare_publish` tests cannot observe.
#[rstest]
#[expect(
    clippy::indexing_slicing,
    reason = "test assertions use .expect() and indexing for clear failure messages"
)]
fn smoke_rust_diagnostics_cleared_once_step_referenced(mut server: ServerHandle) {
    let dir = server.workspace_root().to_path_buf();

    let rust_path = dir.join("rust_clearing_steps.rs");
    std::fs::write(
        &rust_path,
        concat!(
            "use rstest_bdd_macros::given;\n",
            "\n",
            "#[given(\"an orphan step\")]\n",
            "fn an_orphan_step() {}\n",
        ),
    )
    .expect("write rust steps");
    let rust_uri = lsp_types::Url::from_file_path(&rust_path).expect("rust URI");
    let rust_uri = rust_uri.as_str().to_owned();

    // Saving the Rust file with no feature referencing the step publishes a
    // non-empty "unused step" diagnostic for the Rust URI.
    did_save(&mut server.stdin, &rust_path);
    let non_empty_uri = rust_uri.clone();
    server
        .receiver
        .recv_notification_matching(
            move |msg| {
                is_non_empty_diagnostics(msg)
                    && msg["params"]["uri"].as_str() == Some(non_empty_uri.as_str())
            },
            MAX_RECV_MESSAGES,
        )
        .expect("expected non-empty rust diagnostics for the unused step");

    // A feature that uses the step removes the "unused" finding.
    //
    // This save's own notification is deliberately not awaited before the
    // re-save below. `didSave` is dispatched to a synchronous handler that
    // indexes and publishes inline, and both notifications travel the same
    // stdin stream, so the server cannot begin the re-save before this save is
    // indexed. Awaiting here would only assert an ordering the transport
    // already guarantees.
    let feature_path = dir.join("rust_clearing.feature");
    std::fs::write(
        &feature_path,
        concat!(
            "Feature: clearing\n",
            "  Scenario: uses the step\n",
            "    Given an orphan step\n",
        ),
    )
    .expect("write feature");
    did_save(&mut server.stdin, &feature_path);

    // Re-saving the Rust file re-publishes it; the step is now used, so its
    // computed diagnostics are empty and an empty array clears the stale one.
    did_save(&mut server.stdin, &rust_path);
    server
        .receiver
        .recv_notification_matching(
            move |msg| {
                msg.get("method").and_then(|m| m.as_str())
                    == Some("textDocument/publishDiagnostics")
                    && msg["params"]["uri"].as_str() == Some(rust_uri.as_str())
                    && msg["params"]["diagnostics"]
                        .as_array()
                        .is_some_and(Vec::is_empty)
            },
            MAX_RECV_MESSAGES,
        )
        .expect("expected empty rust diagnostics clearing the resolved step");

    shutdown_and_exit(&mut server.stdin, &server.receiver, &mut server.child, 99);
}
