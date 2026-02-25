//! Behavioural tests for harness adapter execution semantics.

use rstest::{fixture, rstest};
use rstest_bdd_harness::{
    HarnessAdapter, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner, StdHarness,
};
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

#[path = "../src/test_utils.rs"]
mod test_utils;

use test_utils::{STD_HARNESS_PANIC_MESSAGE, panic_payload_matches};

#[fixture]
fn default_metadata() -> ScenarioMetadata {
    ScenarioMetadata::default()
}

#[derive(Debug, Default)]
struct MetadataProbeHarness {
    inner: StdHarness,
}

impl HarnessAdapter for MetadataProbeHarness {
    fn run<T>(&self, request: ScenarioRunRequest<'_, T>) -> T {
        let (metadata, runner) = request.into_parts();
        let metadata_for_assertions = metadata.clone();
        let wrapped_request = ScenarioRunRequest::new(
            metadata,
            ScenarioRunner::new(move || {
                assert_eq!(
                    metadata_for_assertions.feature_path(),
                    "tests/features/payment.feature"
                );
                assert_eq!(metadata_for_assertions.scenario_name(), "Payment succeeds");
                assert_eq!(metadata_for_assertions.scenario_line(), 27);
                assert_eq!(metadata_for_assertions.tags(), ["@smoke", "@payments"]);
                runner.run()
            }),
        );
        self.inner.run(wrapped_request)
    }
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
    let harness = MetadataProbeHarness::default();
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

#[rstest]
fn std_harness_propagates_runner_panics(default_metadata: ScenarioMetadata) {
    let request = ScenarioRunRequest::new(
        default_metadata,
        ScenarioRunner::new(|| panic!("{STD_HARNESS_PANIC_MESSAGE}")),
    );
    let harness = StdHarness::new();
    let panic_result = catch_unwind(AssertUnwindSafe(|| harness.run(request)));

    match panic_result {
        Ok(_) => panic!("expected StdHarness to propagate runner panic"),
        Err(payload) => {
            assert!(panic_payload_matches(&*payload, STD_HARNESS_PANIC_MESSAGE));
        }
    }
}
