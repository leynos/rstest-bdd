//! Behavioural tests for GPUI harness adapter execution semantics.
#![cfg(feature = "native-gpui-tests")]

use rstest::{fixture, rstest};
use rstest_bdd_harness::{
    HarnessAdapter, HarnessError, HarnessResult, ScenarioMetadata, ScenarioRunRequest,
    ScenarioRunner, StdScenarioRunRequest, StdScenarioRunner,
};
use rstest_bdd_harness_gpui::GpuiHarness;
use std::cell::Cell;
use std::io;
use std::rc::Rc;

/// Runs a [`GpuiHarness`] with `request`, returning the runner's output.
///
/// Panics with a diagnostic message if the harness returns an error, keeping
/// individual tests free of this boilerplate.
fn run_gpui_harness<T>(request: ScenarioRunRequest<'_, gpui::TestAppContext, T>) -> T {
    let harness = GpuiHarness::new();
    match harness.run(request) {
        Ok(result) => result,
        Err(err) => panic!("gpui harness should not fail: {err}"),
    }
}

#[fixture]
fn default_metadata() -> ScenarioMetadata {
    ScenarioMetadata::default()
}

#[rstest]
fn gpui_harness_executes_runner_once(default_metadata: ScenarioMetadata) {
    let call_count = Rc::new(Cell::new(0u8));
    let call_count_clone = Rc::clone(&call_count);
    let request = ScenarioRunRequest::new(
        default_metadata,
        ScenarioRunner::new(move |_context: gpui::TestAppContext| {
            call_count_clone.set(call_count_clone.get() + 1);
            "done"
        }),
    );

    let result = run_gpui_harness(request);
    assert_eq!(result, "done");
    assert_eq!(call_count.get(), 1);
}

#[rstest]
fn gpui_harness_run_returns_ok(default_metadata: ScenarioMetadata) {
    let request = ScenarioRunRequest::new(
        default_metadata,
        ScenarioRunner::new(|_context: gpui::TestAppContext| "ok"),
    );

    let harness = GpuiHarness::new();
    let Ok(value) = harness.run(request) else {
        panic!("gpui harness should not fail");
    };
    assert_eq!(value, "ok");
}

#[derive(Debug)]
struct GpuiRuntimeBuildFailureProbeHarness;

impl HarnessAdapter for GpuiRuntimeBuildFailureProbeHarness {
    type Context = ();

    fn run<T>(&self, _request: StdScenarioRunRequest<'_, T>) -> HarnessResult<T> {
        Err(HarnessError::RuntimeBuildFailed(io::Error::other(
            "gpui probe failure",
        )))
    }
}

#[rstest]
fn gpui_harness_error_path_propagates_runtime_build_failed(default_metadata: ScenarioMetadata) {
    let request = StdScenarioRunRequest::new(
        default_metadata,
        StdScenarioRunner::new_without_context(|| "unreachable"),
    );
    let harness = GpuiRuntimeBuildFailureProbeHarness;
    let result = harness.run(request);

    let Err(HarnessError::RuntimeBuildFailed(err)) = result else {
        panic!("expected RuntimeBuildFailed, got {result:?}");
    };
    let err = HarnessError::RuntimeBuildFailed(err);
    assert_eq!(
        format!("{err}"),
        "failed to build runtime: gpui probe failure"
    );
}

#[rstest]
fn gpui_harness_supports_non_static_runner_borrows(default_metadata: ScenarioMetadata) {
    let mut counter = 0u8;
    let request = ScenarioRunRequest::new(
        default_metadata,
        ScenarioRunner::new(|_context: gpui::TestAppContext| {
            counter += 1;
            counter
        }),
    );

    let result = run_gpui_harness(request);
    assert_eq!(result, 1);
    assert_eq!(counter, 1);
}

#[test]
fn gpui_context_is_active_inside_harness() {
    let request = ScenarioRunRequest::new(
        ScenarioMetadata::default(),
        ScenarioRunner::new(|context: gpui::TestAppContext| {
            context.test_function_name().is_none() && !context.did_prompt_for_new_path()
        }),
    );
    let result = run_gpui_harness(request);
    assert!(result);
}

#[test]
fn gpui_harness_passes_metadata_through() {
    let request = ScenarioRunRequest::new(
        ScenarioMetadata::new(
            "tests/features/payment.feature",
            "Payment succeeds",
            27,
            vec!["@smoke".to_string(), "@payments".to_string()],
        ),
        ScenarioRunner::new(|_context: gpui::TestAppContext| 200),
    );
    assert_eq!(
        request.metadata().feature_path(),
        "tests/features/payment.feature"
    );
    assert_eq!(request.metadata().scenario_name(), "Payment succeeds");
    let result = run_gpui_harness(request);
    assert_eq!(result, 200);
}
