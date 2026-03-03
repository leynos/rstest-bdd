//! Behavioural tests for harness adapter execution semantics.

use rstest::{fixture, rstest};
use rstest_bdd_harness::{
    HarnessAdapter, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner, StdHarness,
    StdScenarioRunRequest, StdScenarioRunner,
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

#[derive(Debug)]
struct MetadataProbeHarness {
    inner: StdHarness,
    expected_metadata: ScenarioMetadata,
}

impl MetadataProbeHarness {
    fn new(expected_metadata: ScenarioMetadata) -> Self {
        Self {
            inner: StdHarness::new(),
            expected_metadata,
        }
    }
}

impl HarnessAdapter for MetadataProbeHarness {
    type Context = ();

    fn run<T>(&self, request: StdScenarioRunRequest<'_, T>) -> T {
        let (metadata, runner) = request.into_parts();
        let metadata_for_assertions = metadata.clone();
        let expected_metadata = self.expected_metadata.clone();
        let wrapped_request = StdScenarioRunRequest::new(
            metadata,
            StdScenarioRunner::new_without_context(move || {
                assert_eq!(metadata_for_assertions, expected_metadata);
                runner.run_without_context()
            }),
        );
        self.inner.run(wrapped_request)
    }
}

#[rstest]
fn std_harness_executes_runner_once(default_metadata: ScenarioMetadata) {
    let call_count = Rc::new(Cell::new(0u8));
    let call_count_clone = Rc::clone(&call_count);
    let request = StdScenarioRunRequest::new(
        default_metadata,
        StdScenarioRunner::new_without_context(move || {
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
    let expected_metadata = ScenarioMetadata::new(
        "tests/features/payment.feature",
        "Payment succeeds",
        27,
        vec!["@smoke".to_string(), "@payments".to_string()],
    );
    let request = StdScenarioRunRequest::new(
        expected_metadata.clone(),
        StdScenarioRunner::new_without_context(|| 200),
    );
    let harness = MetadataProbeHarness::new(expected_metadata);
    assert_eq!(harness.run(request), 200);
}

#[rstest]
fn std_harness_supports_non_static_runner_borrows(default_metadata: ScenarioMetadata) {
    let mut counter = 0u8;
    let request = StdScenarioRunRequest::new(
        default_metadata,
        StdScenarioRunner::new_without_context(|| {
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
    let request = StdScenarioRunRequest::new(
        default_metadata,
        StdScenarioRunner::new_without_context(|| panic!("{STD_HARNESS_PANIC_MESSAGE}")),
    );
    let harness = StdHarness::new();
    let panic_result = catch_unwind(AssertUnwindSafe(|| harness.run(request)));
    let payload = panic_result.expect_err("expected StdHarness to propagate runner panic");
    assert!(panic_payload_matches(&*payload, STD_HARNESS_PANIC_MESSAGE));
}

#[derive(Debug, Default)]
struct ContextValueHarness;

impl HarnessAdapter for ContextValueHarness {
    type Context = u32;

    fn run<T>(&self, request: ScenarioRunRequest<'_, Self::Context, T>) -> T {
        request.run(42)
    }
}

#[test]
fn harness_can_supply_non_unit_context() {
    let request = ScenarioRunRequest::new(
        ScenarioMetadata::default(),
        ScenarioRunner::new(|context: u32| context + 1),
    );
    let harness = ContextValueHarness;
    assert_eq!(harness.run(request), 43);
}
