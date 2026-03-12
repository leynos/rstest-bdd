//! Behavioural tests for GPUI harness adapter execution semantics.
#![cfg(feature = "native-gpui-tests")]

use rstest::{fixture, rstest};
use rstest_bdd_harness::{HarnessAdapter, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner};
use rstest_bdd_harness_gpui::GpuiHarness;
use std::cell::Cell;
use std::rc::Rc;

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

    let harness = GpuiHarness::new();
    assert_eq!(harness.run(request), "done");
    assert_eq!(call_count.get(), 1);
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

    let harness = GpuiHarness::new();
    assert_eq!(harness.run(request), 1);
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
    let harness = GpuiHarness::new();
    assert!(harness.run(request));
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
    let harness = GpuiHarness::new();
    assert_eq!(harness.run(request), 200);
}
