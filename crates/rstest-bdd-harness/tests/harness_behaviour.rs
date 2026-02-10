//! Behavioural tests for harness adapter execution semantics.

use rstest::{fixture, rstest};
use rstest_bdd_harness::{
    HarnessAdapter, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner, StdHarness,
};
use std::cell::Cell;
use std::rc::Rc;

struct InspectHarness;

impl HarnessAdapter for InspectHarness {
    fn run<T>(request: ScenarioRunRequest<'_, T>) -> T {
        assert_eq!(
            request.metadata().feature_path(),
            "tests/features/payment.feature"
        );
        assert_eq!(request.metadata().scenario_name(), "Payment succeeds");
        assert_eq!(request.metadata().scenario_line(), 27);
        assert_eq!(request.metadata().tags(), ["@smoke", "@payments"]);
        request.run()
    }
}

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

    assert_eq!(StdHarness::run(request), "done");
    assert_eq!(call_count.get(), 1);
}

#[test]
fn custom_harness_can_inspect_metadata_before_running() {
    let request = ScenarioRunRequest::new(
        ScenarioMetadata::new(
            "tests/features/payment.feature",
            "Payment succeeds",
            27,
            vec!["@smoke".to_string(), "@payments".to_string()],
        ),
        ScenarioRunner::new(|| 200),
    );
    assert_eq!(InspectHarness::run(request), 200);
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

    assert_eq!(StdHarness::run(request), 1);
    assert_eq!(counter, 1);
}
