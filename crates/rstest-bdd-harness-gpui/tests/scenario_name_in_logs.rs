//! Regression coverage for scenario-name diagnostics in `GpuiHarness`.
//!
//! These tests prove that when a step running under `GpuiHarness` panics, the
//! resumed payload carries the originating feature path, scenario name, and
//! feature-file line number so developers can orientate failures quickly.
#![cfg(feature = "native-gpui-tests")]

use std::{
    panic::{AssertUnwindSafe, catch_unwind, panic_any},
    process::Command,
    sync::{Arc, Mutex},
};

use rstest::rstest;
use rstest_bdd::panic_message;
use rstest_bdd_harness::{
    HarnessAdapter,
    HarnessResult,
    ScenarioMetadata,
    ScenarioRunRequest,
    ScenarioRunner,
};
use rstest_bdd_harness_gpui::GpuiHarness;
use serial_test::serial;
use tracing_subscriber::{Registry, prelude::*};

mod support;

use support::{RecordingLayer, configured_snapshot_settings};
const FEATURE_PATH: &str = "tests/features/scenario_name_in_logs.feature";
const FAILING_SCENARIO: &str = "Step panics with augmented diagnostic";
const SCENARIO_LINE: u32 = 7;
const STEP_PANIC: &str = "step panic without scenario context";
const STDERR_CHILD_ENV: &str = "RSTEST_BDD_GPUI_ASSERT_STDERR_CHILD";
const TRACING_CHILD_ENV: &str = "RSTEST_BDD_GPUI_ASSERT_TRACING_CHILD";

/// Asserts that a successful scenario run returns its output value without
/// any panic or error marker.
#[rstest]
#[serial]
fn successful_scenario_returns_without_failure_marker() {
    let request = ScenarioRunRequest::new(
        scenario_metadata("Successful scenario runs cleanly"),
        ScenarioRunner::new(|_context: gpui::TestAppContext| "ok"),
    );

    let Ok(result) = run_scenario(request) else {
        panic!("gpui harness should not fail");
    };

    assert_eq!(result, "ok");
}

/// Asserts that the augmented panic message from a failing step includes
/// the originating feature path, scenario name, and step line number.
#[rstest]
#[serial]
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
#[serial]
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

    if let Err(error) = tracing::subscriber::set_global_default(subscriber) {
        panic!("child process should install tracing subscriber once: {error}");
    }
    let _message = catch_scenario_panic(request);

    let Ok(events) = events.lock() else {
        panic!("captured tracing events should not be poisoned");
    };
    let Some(event) = events
        .iter()
        .find(|event| event.contains("GPUI scenario panicked"))
    else {
        panic!("expected GPUI panic tracing event, got: {events:?}");
    };
    configured_snapshot_settings().bind(|| insta::assert_snapshot!(event));
}

/// Asserts that when a failing scenario is run in a child process, its
/// augmented panic diagnostic appears on stderr.
#[rstest]
#[serial]
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
    let Ok(stderr) = String::from_utf8(output.stderr) else {
        panic!("stderr should be UTF-8");
    };
    configured_snapshot_settings().bind(|| insta::assert_snapshot!(&stderr));
}

/// Runs an assertion in a child process so stderr/tracing capture is isolated.
fn run_child_assertion(
    test_name: &str,
    child_env: &str,
    expect_success: bool,
) -> std::process::Output {
    let Ok(current_exe) = std::env::current_exe() else {
        panic!("test binary path is available");
    };
    let Ok(output) = Command::new(current_exe)
        .arg(test_name)
        .arg("--exact")
        .arg("--nocapture")
        .env(child_env, "1")
        // The child's stderr is snapshotted, and the panic hook prints either
        // a backtrace or the "run with RUST_BACKTRACE=1" note depending on the
        // ambient environment. Coverage runs retain debuginfo and some runners
        // export RUST_BACKTRACE, so pin it here to keep the captured stderr
        // identical across lanes. `configured_snapshot_settings` normalizes the
        // other direction, should a caller override this.
        .env("RUST_BACKTRACE", "0")
        .output()
    else {
        panic!("child test process should run");
    };

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
#[serial]
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

    let Ok(result) = run_scenario(next_request) else {
        panic!("gpui harness should not fail");
    };

    assert_eq!(result, "fresh");
}

/// Builds a [`ScenarioMetadata`] with the hard-coded feature path, line
/// number, and a `@regression` tag, using `name` as the scenario name.
fn scenario_metadata(name: &str) -> ScenarioMetadata {
    ScenarioMetadata::new(
        FEATURE_PATH,
        name,
        SCENARIO_LINE,
        vec!["@regression".to_owned()],
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

/// Builds a scenario request that always panics with [`STEP_PANIC`].
fn failing_scenario_request() -> ScenarioRunRequest<'static, gpui::TestAppContext, ()> {
    ScenarioRunRequest::new(
        scenario_metadata(FAILING_SCENARIO),
        ScenarioRunner::new(|_context: gpui::TestAppContext| {
            panic!("{STEP_PANIC}");
        }),
    )
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
#[serial]
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

/// Asserts that the augmented panic message includes the scenario name
/// whether the original panic payload is an owned `String` or a `&str`.
///
/// Each case exercises a distinct downcast path in
/// `augmented_panic_message`:
/// - `string_payload`: the owned-`String` downcast.
/// - `str_payload`: the `&str` downcast.
#[rstest]
#[case::string_payload(
    "String payload scenario",
    Box::new(|| -> () { panic_any("a string panic".to_owned()); }) as Box<dyn Fn() + Send + 'static>,
)]
#[case::str_payload(
    "&str payload scenario",
    Box::new(|| -> () { panic!("a &str panic"); }) as Box<dyn Fn() + Send + 'static>,
)]
#[serial]
fn augmented_message_includes_scenario_name_for_payload_type(
    #[case] scenario_name: &str,
    #[case] panic_fn: Box<dyn Fn() + Send + 'static>,
) {
    let request = ScenarioRunRequest::new(
        scenario_metadata(scenario_name),
        ScenarioRunner::new(move |_context: gpui::TestAppContext| {
            panic_fn();
        }),
    );

    let message = catch_scenario_panic(request);

    let mut settings = configured_snapshot_settings();
    settings.set_snapshot_suffix(scenario_name);
    settings.bind(|| insta::assert_snapshot!(&message));
}

/// Asserts that the augmented panic message includes the scenario name when
/// the original panic payload is an opaque `Any` value that is neither
/// `String` nor `&str`.
#[rstest]
#[serial]
fn augmented_message_includes_scenario_name_for_opaque_any_payload() {
    #[derive(Debug)]
    #[expect(
        dead_code,
        reason = "field only exists to produce an opaque Any payload; see ExecPlan 10.1.4"
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
#[serial]
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
        let Ok(stderr) = String::from_utf8(output.stderr) else {
            panic!("stderr should be UTF-8");
        };
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
