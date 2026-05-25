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

    assert!(
        message.contains(FAILING_SCENARIO),
        "expected scenario name in augmented diagnostic, got: {message}",
    );
    assert!(
        message.contains(FEATURE_PATH),
        "expected feature path in augmented diagnostic, got: {message}",
    );
    assert!(
        message.contains(":7"),
        "expected scenario line in augmented diagnostic, got: {message}",
    );
    assert!(
        message.contains(STEP_PANIC),
        "expected original panic message preserved, got: {message}",
    );
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
    assert_augmented_diagnostic(event);
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
    assert_augmented_diagnostic(&stderr);
}

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

fn scenario_metadata(name: &str) -> ScenarioMetadata {
    ScenarioMetadata::new(
        FEATURE_PATH,
        name,
        SCENARIO_LINE,
        vec!["@regression".to_string()],
    )
}

fn run_scenario<T>(request: ScenarioRunRequest<'_, gpui::TestAppContext, T>) -> HarnessResult<T> {
    GpuiHarness::new().run(request)
}

fn catch_scenario_panic<T>(request: ScenarioRunRequest<'_, gpui::TestAppContext, T>) -> String {
    let result = catch_unwind(AssertUnwindSafe(|| run_scenario(request)));
    let Err(payload) = result else {
        panic!("expected GpuiHarness to propagate scenario panic");
    };
    panic_message(payload.as_ref())
}

fn failing_scenario_request() -> ScenarioRunRequest<'static, gpui::TestAppContext, ()> {
    ScenarioRunRequest::new(
        scenario_metadata(FAILING_SCENARIO),
        ScenarioRunner::new(|_context: gpui::TestAppContext| {
            panic!("{STEP_PANIC}");
        }),
    )
}

fn assert_augmented_diagnostic(message: &str) {
    assert!(
        message.contains(FAILING_SCENARIO),
        "expected scenario name in augmented diagnostic, got: {message}",
    );
    assert!(
        message.contains(FEATURE_PATH),
        "expected feature path in augmented diagnostic, got: {message}",
    );
    assert!(
        message.contains(":7") || message.contains("scenario_line=7"),
        "expected scenario line in augmented diagnostic, got: {message}",
    );
    assert!(
        message.contains(STEP_PANIC),
        "expected original panic message preserved, got: {message}",
    );
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
