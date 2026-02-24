//! Behavioural tests for harness adapter execution semantics.

use rstest::{fixture, rstest};
use rstest_bdd_harness::{
    HarnessAdapter, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner, StdHarness,
};
use std::cell::Cell;
use std::rc::Rc;

#[fixture]
fn default_metadata() -> ScenarioMetadata {
    ScenarioMetadata::default()
}

#[rstest]
fn std_harness_executes_runner_once(default_metadata: ScenarioMetadata) {
    let call_count = Rc::new(Cell::new(0u8));
    let call_count_clone = Rc::clone(&call_count);
    let request = ScenarioRunRequest::new(
        default_metadata,
        ScenarioRunner::new(move || {
            call_count_clone.set(call_count_clone.get() + 1);
            "done"
        }),
    );

    let harness = StdHarness::new();
    assert_eq!(harness.run(request), "done");
    assert_eq!(call_count.get(), 1);
}

#[test]
fn std_harness_passes_metadata_through() {
    let request = ScenarioRunRequest::new(
        ScenarioMetadata::new(
            "tests/features/payment.feature",
            "Payment succeeds",
            27,
            vec!["@smoke".to_string(), "@payments".to_string()],
        ),
        ScenarioRunner::new(|| 200),
    );
    assert_eq!(
        request.metadata().feature_path(),
        "tests/features/payment.feature"
    );
    assert_eq!(request.metadata().scenario_name(), "Payment succeeds");
    assert_eq!(request.metadata().scenario_line(), 27);
    assert_eq!(request.metadata().tags(), ["@smoke", "@payments"]);
    let harness = StdHarness::new();
    assert_eq!(harness.run(request), 200);
}

#[rstest]
fn std_harness_supports_non_static_runner_borrows(default_metadata: ScenarioMetadata) {
    let mut counter = 0u8;
    let request = ScenarioRunRequest::new(
        default_metadata,
        ScenarioRunner::new(|| {
            counter += 1;
            counter
        }),
    );

    let harness = StdHarness::new();
    assert_eq!(harness.run(request), 1);
    assert_eq!(counter, 1);
}

#[test]
#[should_panic(expected = "std harness panic propagation")]
fn std_harness_propagates_runner_panics() {
    let request = ScenarioRunRequest::new(
        ScenarioMetadata::default(),
        ScenarioRunner::new(|| panic!("std harness panic propagation")),
    );
    let harness = StdHarness::new();
    harness.run(request);
}
