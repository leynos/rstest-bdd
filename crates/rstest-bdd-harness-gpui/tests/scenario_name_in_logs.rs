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
use std::panic::{AssertUnwindSafe, catch_unwind};

const FEATURE_PATH: &str = "tests/features/scenario_name_in_logs.feature";
const FAILING_SCENARIO: &str = "Step panics with augmented diagnostic";
const SCENARIO_LINE: u32 = 7;
const STEP_PANIC: &str = "step panic without scenario context";

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
