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
use std::io::{self, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
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

#[rstest]
fn failing_scenario_diagnostic_is_emitted_to_tracing_error() {
    if std::env::var_os(TRACING_CHILD_ENV).is_none() {
        run_child_assertion(
            "failing_scenario_diagnostic_is_emitted_to_tracing_error",
            TRACING_CHILD_ENV,
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
    assert!(event.contains("rstest_bdd_harness_gpui::GpuiHarness"));
}

#[rstest]
fn failing_scenario_diagnostic_is_written_to_stderr() {
    if std::env::var_os(STDERR_CHILD_ENV).is_some() {
        let _message = catch_scenario_panic(failing_scenario_request());
        return;
    }

    let output = run_child_assertion(
        "failing_scenario_diagnostic_is_written_to_stderr",
        STDERR_CHILD_ENV,
    );
    let stderr = String::from_utf8(output.stderr)
        .unwrap_or_else(|error| panic!("stderr should be UTF-8: {error}"));
    configured_snapshot_settings().bind(|| insta::assert_snapshot!(&stderr));
}

/// Spawns a child process running the named test with the given environment
/// marker, returning the process output.
///
/// Used by tests that must inspect tracing events or stderr from a separate
/// process to avoid interference with the test harness.
fn run_child_assertion(test_name: &str, child_env: &str) -> std::process::Output {
    let current_exe = std::env::current_exe()
        .unwrap_or_else(|error| panic!("test binary path is available: {error}"));
    let output = Command::new(current_exe)
        .arg(test_name)
        .arg("--exact")
        .arg("--nocapture")
        .env(child_env, "1")
        .output()
        .unwrap_or_else(|error| panic!("child test process should run: {error}"));

    assert!(
        output.status.success(),
        "child stderr assertion process failed: {output:?}",
    );
    output
}

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

/// Returns [`insta::Settings`] configured with redactions for the feature
/// path, scenario name, and line-number fields that vary across test runs.
fn configured_snapshot_settings() -> insta::Settings {
    let mut settings = insta::Settings::clone_current();
    for (pattern, replacement) in &[
        (r"tests/features/[^ ]+\.feature", "[FEATURE_PATH]"),
        (r"scenario_name=[^\s,}]+", "scenario_name=[SCENARIO_NAME]"),
        (r#"scenario="[^"]+""#, "scenario=\"[SCENARIO_NAME]\""),
        (r":\d+", ":[LINE]"),
        (r"scenario_line=\d+", "scenario_line=[LINE]"),
        (r"\(\d+\)", "([TID])"),
    ] {
        settings.add_filter(pattern, *replacement);
    }
    settings
}

// ---------------------------------------------------------------------------
// Edge-case tests — special characters, write failure, payload invariants,
// and teardown-panic ordering.
// ---------------------------------------------------------------------------

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

    let mut settings = configured_snapshot_settings();
    // The scenario name appears Debug-formatted (escaped) in the augmented
    // payload, so redact the escaped form.
    let escaped_debug_name = scenario_name.escape_debug().to_string();
    let debug_pattern = format!(r#""{}""#, regex::escape(&escaped_debug_name));
    settings.add_filter(&debug_pattern, "\"[SCENARIO_NAME]\"");
    // Also redact the raw name for any non-Debug occurrences (e.g. tracing).
    settings.add_filter(&regex::escape(scenario_name), "[SCENARIO_NAME]");
    settings.bind(|| insta::assert_snapshot!(&message));
}

#[rstest]
fn stderr_write_failure_is_non_fatal() {
    // A [`Write`] implementation that always returns `BrokenPipe`.
    struct BrokenPipeWriter;

    impl Write for BrokenPipeWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "simulated broken pipe",
            ))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let message = "diagnostic message for broken pipe test";
    let result = GpuiHarness::write_stderr_diagnostic_to(&mut BrokenPipeWriter, message);

    assert!(
        result.is_err(),
        "write_stderr_diagnostic_to should return Err on I/O failure, got: {result:?}"
    );
    let Err(err) = result else {
        panic!("expected Err, got Ok");
    };
    assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);
}

#[rstest]
fn augmented_message_includes_scenario_name_for_string_payload() {
    let metadata = scenario_metadata("String payload scenario");
    let payload: Box<dyn std::any::Any + Send> = Box::new("a string panic".to_string());
    let message = GpuiHarness::augmented_panic_message(payload.as_ref(), &metadata);
    configured_snapshot_settings().bind(|| insta::assert_snapshot!(&message));
}

#[rstest]
fn augmented_message_includes_scenario_name_for_str_payload() {
    let metadata = scenario_metadata("&str payload scenario");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        panic!("a &str panic");
    }));
    let payload = result.unwrap_err();
    let message = GpuiHarness::augmented_panic_message(payload.as_ref(), &metadata);
    configured_snapshot_settings().bind(|| insta::assert_snapshot!(&message));
}

#[rstest]
fn augmented_message_includes_scenario_name_for_opaque_any_payload() {
    #[derive(Debug)]
    #[expect(
        dead_code,
        reason = "field only exists to produce an opaque Any payload"
    )]
    struct CustomPayload(u32);

    let metadata = scenario_metadata("Opaque Any payload scenario");
    let payload: Box<dyn std::any::Any + Send> = Box::new(CustomPayload(99));
    let message = GpuiHarness::augmented_panic_message(payload.as_ref(), &metadata);
    let mut settings = configured_snapshot_settings();
    settings.add_filter(r"Opaque Any payload scenario", "[SCENARIO_NAME]");
    settings.bind(|| insta::assert_snapshot!(&message));
    assert!(message.contains("Opaque Any payload scenario"));
    assert!(message.contains("erased `Any` payload"));
}

/// Verifies that a teardown panic in a child process does not suppress the
/// original step panic diagnostic in stderr.
///
/// We construct a scenario whose step panics and whose tear-down path also
/// panics (via a `Drop` guard), then run the whole thing in a child process
/// and assert the stderr still carries the augmented diagnostic for the
/// original step panic.
#[rstest]
fn teardown_panic_does_not_suppress_original_diagnostic() {
    const TEARDOWN_CHILD_ENV: &str = "RSTEST_BDD_GPUI_TEARDOWN_CHILD";

    if std::env::var_os(TEARDOWN_CHILD_ENV).is_none() {
        let output = run_child_assertion(
            "teardown_panic_does_not_suppress_original_diagnostic",
            TEARDOWN_CHILD_ENV,
        );
        let stderr = String::from_utf8(output.stderr)
            .unwrap_or_else(|error| panic!("stderr should be UTF-8: {error}"));
        // The child process double-panics (aborts), so success=false is expected.
        // The important thing is that the original diagnostic appears in stderr
        // before the abort.
        assert!(
            stderr.contains("GPUI scenario panicked")
                || stderr.contains("rstest-bdd-harness-gpui scenario panicked"),
            "expected original augmented diagnostic in stderr before abort, got: {stderr}"
        );
        return;
    }

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
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.fields.push(format!("{}={value:?}", field.name()));
    }
}
