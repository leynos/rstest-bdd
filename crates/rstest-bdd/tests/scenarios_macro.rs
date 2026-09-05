//! Behavioural tests for the `scenarios!` macro.

use rstest_bdd_harness::{HarnessAdapter, HarnessError, StdScenarioRunRequest};
use rstest_bdd_macros::{given, scenarios, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[when("an action occurs with {n}")]
fn action_with_num(n: i32) { let _ = n; }

#[then("events are recorded")]
fn events_recorded() {}

#[then("only fast examples run")]
fn only_fast_examples_run(num: &'static str) {
    assert_eq!(num, "1", "unexpected example row executed");
}

#[when("a slow action occurs")]
fn slow_action_occurs() {
    panic!("slow scenario should be filtered out");
}

#[then("slow events are recorded")]
fn slow_events_recorded() {
    panic!("slow scenario should be filtered out");
}

/// Captures the generated metadata contract for the filtered `scenarios!` run.
#[derive(Default)]
struct ManifestRelativeMetadataHarness;

impl HarnessAdapter for ManifestRelativeMetadataHarness {
    type Context = ();

    fn run<T>(&self, request: StdScenarioRunRequest<'_, T>) -> Result<T, HarnessError> {
        let metadata = request.metadata();
        let expected_feature_path = match metadata.scenario_name() {
            "fast macro scenario" => "tests/features/filtered/fast.feature",
            "outline example" => "tests/features/filtered/mixed.feature",
            name => panic!("unexpected filtered scenario metadata: {name}"),
        };
        assert_eq!(metadata.feature_path(), expected_feature_path);
        Ok(request.run_without_context())
    }
}

scenarios!("tests/features/auto");
scenarios!(
    "tests/features/filtered",
    tags = "@fast",
    harness = ManifestRelativeMetadataHarness,
);
