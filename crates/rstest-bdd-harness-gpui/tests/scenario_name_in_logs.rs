//! Regression coverage for scenario-name diagnostics in `GpuiHarness`.
//!
//! These tests prove that when a step running under `GpuiHarness` panics, the
//! resumed payload carries the originating feature path, scenario name, and
//! feature-file line number so developers can orientate failures quickly.
#![cfg(feature = "native-gpui-tests")]

use rstest::rstest;
use rstest_bdd::panic_message;
use rstest_bdd_harness::{
    HarnessAdapter, HarnessResult, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner,
};
use rstest_bdd_harness_gpui::GpuiHarness;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind, panic_any};
use std::process::Command;
use std::sync::{Arc, Mutex};
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{Layer, Registry};

const FEATURE_PATH: &str = "tests/features/scenario_name_in_logs.feature";
const FAILING_SCENARIO: &str = "Step panics with augmented diagnostic";
const SCENARIO_LINE: u32 = 7;
const STEP_PANIC: &str = "step panic without scenario context";
const STDERR_CHILD_ENV: &str = "RSTEST_BDD_GPUI_ASSERT_STDERR_CHILD";
const TRACING_CHILD_ENV: &str = "RSTEST_BDD_GPUI_ASSERT_TRACING_CHILD";

/// Asserts that a successful scenario run returns its output value without
/// any panic or error marker.
#[rstest]
fn successful_scenario_returns_without_failure_marker() {
    let request = ScenarioRunRequest::new(
        scenario_metadata("Successful scenario runs cleanly"),
        ScenarioRunner::new(|_context: gpui::TestAppContext| "ok"),
    );

    let result =
        run_scenario(request).unwrap_or_else(|err| panic!("gpui harness should not fail: {err}"));

    assert_eq!(result, "ok");
}

/// Asserts that the augmented panic message from a failing step includes
/// the originating feature path, scenario name, and step line number.
#[rstest]
fn failing_scenario_diagnostic_includes_scenario_name() {
    let request = ScenarioRunRequest::new(
        scenario_metadata(FAILING_SCENARIO),
        ScenarioRunner::new(|_context: gpui::TestAppContext| {
            panic!("{STEP_PANIC}");
        }),
    );

    let message = catch_scenario_panic(request);
    configured_snapshot_settings().bind(|| insta::assert_snapshot!(&message));
}

/// Asserts that when a failing scenario is run in a child process, its
/// augmented panic diagnostic appears in a `tracing::error!` event.
#[rstest]
fn failing_scenario_diagnostic_is_emitted_to_tracing_error() {
    if std::env::var_os(TRACING_CHILD_ENV).is_none() {
        run_child_assertion(
            "failing_scenario_diagnostic_is_emitted_to_tracing_error",
            TRACING_CHILD_ENV,
            true,
        );
        return;
    }

    let events = Arc::new(Mutex::new(Vec::new()));
    let subscriber = Registry::default().with(RecordingLayer {
        events: Arc::clone(&events),
    });
    let request = failing_scenario_request();

    tracing::subscriber::set_global_default(subscriber).unwrap_or_else(|error| {
        panic!("child process should install tracing subscriber once: {error}");
    });
    let _message = catch_scenario_panic(request);

    let events = events.lock().unwrap_or_else(|error| {
        panic!("captured tracing events should not be poisoned: {error}");
    });
    let event = events
        .iter()
        .find(|event| event.contains("GPUI scenario panicked"))
        .unwrap_or_else(|| panic!("expected GPUI panic tracing event, got: {events:?}"));
    configured_snapshot_settings().bind(|| insta::assert_snapshot!(event));
}

/// Asserts that when a failing scenario is run in a child process, its
/// augmented panic diagnostic appears on stderr.
#[rstest]
fn failing_scenario_diagnostic_is_written_to_stderr() {
    if std::env::var_os(STDERR_CHILD_ENV).is_some() {
        let _message = catch_scenario_panic(failing_scenario_request());
        return;
    }

    let output = run_child_assertion(
        "failing_scenario_diagnostic_is_written_to_stderr",
        STDERR_CHILD_ENV,
        true,
    );
    let stderr = String::from_utf8(output.stderr)
        .unwrap_or_else(|error| panic!("stderr should be UTF-8: {error}"));
    configured_snapshot_settings().bind(|| insta::assert_snapshot!(&stderr));
}

/// Spawns a child process running the named test with the given environment
/// marker, returning the process output.
///
/// If `expect_success` is true, panics when the child exits non-zero. Set it
/// to false for tests that exercise double-panics or deliberate aborts.
///
/// Used by tests that must inspect tracing events or stderr from a separate
/// process to avoid interference with the test harness.
fn run_child_assertion(
    test_name: &str,
    child_env: &str,
    expect_success: bool,
) -> std::process::Output {
    let current_exe = std::env::current_exe()
        .unwrap_or_else(|error| panic!("test binary path is available: {error}"));
    let output = Command::new(current_exe)
        .arg(test_name)
        .arg("--exact")
        .arg("--nocapture")
        .env(child_env, "1")
        .output()
        .unwrap_or_else(|error| panic!("child test process should run: {error}"));

    if expect_success {
        assert!(
            output.status.success(),
            "child stderr assertion process failed: {output:?}",
        );
    }
    output
}

/// Verifies that after a scenario panics, a subsequent scenario executes
/// with a fresh GPUI context and is not contaminated by the prior failure.
#[rstest]
fn second_scenario_after_failure_runs_with_fresh_context() {
    let failing_request = ScenarioRunRequest::new(
        scenario_metadata(FAILING_SCENARIO),
        ScenarioRunner::new(|_context: gpui::TestAppContext| {
            panic!("{STEP_PANIC}");
        }),
    );
    let _message = catch_scenario_panic(failing_request);

    let next_request = ScenarioRunRequest::new(
        scenario_metadata("Fresh scenario after failure"),
        ScenarioRunner::new(|_context: gpui::TestAppContext| "fresh"),
    );

    let result = run_scenario(next_request)
        .unwrap_or_else(|err| panic!("gpui harness should not fail: {err}"));

    assert_eq!(result, "fresh");
}

/// Builds a [`ScenarioMetadata`] with the hard-coded feature path, line
/// number, and a `@regression` tag, using `name` as the scenario name.
fn scenario_metadata(name: &str) -> ScenarioMetadata {
    ScenarioMetadata::new(
        FEATURE_PATH,
        name,
        SCENARIO_LINE,
        vec!["@regression".to_string()],
    )
}

/// Runs a scenario request through a fresh [`GpuiHarness`].
fn run_scenario<T>(request: ScenarioRunRequest<'_, gpui::TestAppContext, T>) -> HarnessResult<T> {
    GpuiHarness::new().run(request)
}

/// Runs a scenario through [`run_scenario`] inside `catch_unwind`, expecting
/// a panic, and returns the rendered panic payload as a string.
fn catch_scenario_panic<T>(request: ScenarioRunRequest<'_, gpui::TestAppContext, T>) -> String {
    let result = catch_unwind(AssertUnwindSafe(|| run_scenario(request)));
    let Err(payload) = result else {
        panic!("expected GpuiHarness to propagate scenario panic");
    };
    panic_message(payload.as_ref())
}

/// Builds a scenario request that always panics with [`STEP_PANIC`].
fn failing_scenario_request() -> ScenarioRunRequest<'static, gpui::TestAppContext, ()> {
    ScenarioRunRequest::new(
        scenario_metadata(FAILING_SCENARIO),
        ScenarioRunner::new(|_context: gpui::TestAppContext| {
            panic!("{STEP_PANIC}");
        }),
    )
}

/// Returns [`insta::Settings`] with redactions for nondeterministic data only.
///
/// Snapshot bodies must pin the exact feature path, scenario name, and feature
/// line so that regressions in the scenario-name diagnostic are caught.  The
/// only redactions applied here cover values that genuinely vary across runs:
/// thread IDs in panic headers, the Rust source file line and column of the
/// panic site, and the `TypeId` hex emitted for opaque payloads.
fn configured_snapshot_settings() -> insta::Settings {
    let mut settings = insta::Settings::clone_current();
    for (pattern, replacement) in &[
        (r"\(\d+\)", "([TID])"),
        (r"\.rs:\d+:\d+", ".rs:[LINE]:[COL]"),
        (r"TypeId\(0x[0-9a-f]+\)", "TypeId([TYPEID])"),
    ] {
        settings.add_filter(pattern, *replacement);
    }
    settings
}

// ---------------------------------------------------------------------------
// Edge-case tests — special characters, payload type coverage, and
// teardown-panic ordering.  Each test routes through the harness so the
// augmented diagnostic is exercised end-to-end without exposing private
// helpers on the public API.
// ---------------------------------------------------------------------------

/// Asserts that Unicode, newline, tab, and shell-special characters in a
/// scenario name are preserved in the augmented panic diagnostic.
#[rstest]
fn special_characters_in_scenario_name_are_preserved_in_diagnostic() {
    let scenario_name = "Unicode 🐇 & newline\nand tab\t";
    let request = ScenarioRunRequest::new(
        scenario_metadata(scenario_name),
        ScenarioRunner::new(|_context: gpui::TestAppContext| {
            panic!("special-character step panic");
        }),
    );

    let message = catch_scenario_panic(request);

    configured_snapshot_settings().bind(|| insta::assert_snapshot!(&message));
}

/// Asserts that the augmented panic message includes the scenario name when
/// the original panic payload is a `String`.
///
/// `panic_any` carries an owned `String` so the harness's panic-message
/// rendering is exercised against the owned-string downcast path.
#[rstest]
fn augmented_message_includes_scenario_name_for_string_payload() {
    let request = ScenarioRunRequest::new(
        scenario_metadata("String payload scenario"),
        ScenarioRunner::new(|_context: gpui::TestAppContext| {
            panic_any("a string panic".to_string());
        }),
    );

    let message = catch_scenario_panic(request);

    configured_snapshot_settings().bind(|| insta::assert_snapshot!(&message));
}

/// Asserts that the augmented panic message includes the scenario name when
/// the original panic payload is a `&'static str`.
///
/// `panic!(literal)` with no format arguments produces a `&'static str`
/// payload, so this exercises the borrowed-str downcast path.
#[rstest]
fn augmented_message_includes_scenario_name_for_str_payload() {
    let request = ScenarioRunRequest::new(
        scenario_metadata("&str payload scenario"),
        ScenarioRunner::new(|_context: gpui::TestAppContext| {
            panic!("a &str panic");
        }),
    );

    let message = catch_scenario_panic(request);

    configured_snapshot_settings().bind(|| insta::assert_snapshot!(&message));
}

/// Asserts that the augmented panic message includes the scenario name when
/// the original panic payload is an opaque `Any` value that is neither
/// `String` nor `&str`.
#[rstest]
fn augmented_message_includes_scenario_name_for_opaque_any_payload() {
    #[derive(Debug)]
    #[expect(
        dead_code,
        reason = "field only exists to produce an opaque Any payload"
    )]
    struct CustomPayload(u32);

    let request = ScenarioRunRequest::new(
        scenario_metadata("Opaque Any payload scenario"),
        ScenarioRunner::new(|_context: gpui::TestAppContext| {
            panic_any(CustomPayload(99));
        }),
    );

    let message = catch_scenario_panic(request);

    configured_snapshot_settings().bind(|| insta::assert_snapshot!(&message));
}

/// Verifies that a teardown panic does not suppress the original step panic
/// diagnostic.  A `Drop` guard panics during unwinding, triggering a
/// double-panic (process abort).  The parent asserts the child exits non-zero
/// and snapshots stderr to confirm the original diagnostic appeared first.
#[rstest]
fn teardown_panic_does_not_suppress_original_diagnostic() {
    const TEARDOWN_CHILD_ENV: &str = "RSTEST_BDD_GPUI_TEARDOWN_CHILD";

    // Drop guard that panics during unwinding, triggering a double-panic.
    struct TeardownGuard;
    impl Drop for TeardownGuard {
        fn drop(&mut self) {
            panic!("teardown-panic ordering guard");
        }
    }

    if std::env::var_os(TEARDOWN_CHILD_ENV).is_none() {
        let output = run_child_assertion(
            "teardown_panic_does_not_suppress_original_diagnostic",
            TEARDOWN_CHILD_ENV,
            false,
        );
        assert!(
            !output.status.success(),
            "expected child process to abort after double-panic, got success"
        );
        let stderr = String::from_utf8(output.stderr)
            .unwrap_or_else(|error| panic!("stderr should be UTF-8: {error}"));
        configured_snapshot_settings().bind(|| insta::assert_snapshot!(&stderr));
        return;
    }

    let _guard = TeardownGuard;
    let request = ScenarioRunRequest::new(
        scenario_metadata("Teardown panic scenario"),
        ScenarioRunner::new(|_context: gpui::TestAppContext| {
            panic!("original step panic");
        }),
    );
    let _message = catch_scenario_panic(request);
}

struct RecordingLayer {
    events: Arc<Mutex<Vec<String>>>,
}

impl<S> Layer<S> for RecordingLayer
where
    S: Subscriber,
{
    /// Visits every tracing event, serialises its fields, and appends the
    /// result to the shared event buffer for later inspection.
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let mut visitor = EventVisitor::default();
        event.record(&mut visitor);
        self.events
            .lock()
            .unwrap_or_else(|error| {
                panic!("captured tracing events should not be poisoned: {error}")
            })
            .push(visitor.fields.join(" "));
    }
}

#[derive(Default)]
struct EventVisitor {
    fields: Vec<String>,
}

impl Visit for EventVisitor {
    /// Records the debug representation of a tracing field into the
    /// accumulated field list.
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.fields.push(format!("{}={value:?}", field.name()));
    }
}
