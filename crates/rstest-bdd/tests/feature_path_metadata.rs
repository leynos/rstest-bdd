//! Behavioural regression tests proving that an expanded *and executed*
//! scenario macro records a manifest-relative `feature_path` at runtime
//! (10.3.3 Decision D3, review follow-up).
//!
//! The existing unit tests pin the token shape at the macro's two
//! `TokenStream2` producers. These tests add the missing runtime proof across
//! the macro-to-reporting boundary: each route expands a macro over a
//! crate-local feature file, executes the generated scenario body, and reads
//! the recorded `ScenarioRecord` from the reporting collector, asserting the
//! exact manifest-relative path with `/` separators. No macro token strings
//! are inspected.
//!
//! Both routes assert from inside the executed generated code, so every test
//! is self-contained: the reporting collector is process-global (see the
//! `reporting` module's concurrency notes), and the assertions use
//! presence-based `snapshot` reads filtered by scenario name, which are
//! insensitive to records from concurrently executing scenarios and survive
//! cargo-nextest's process-per-test scheduling.

use rstest_bdd::reporting;
use rstest_bdd_harness::{HarnessAdapter, HarnessError, StdScenarioRunRequest};
use rstest_bdd_macros::{given, scenario, scenarios, then, when};

// ---------------------------------------------------------------------------
// Step definitions shared by both macro routes.
// ---------------------------------------------------------------------------

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[when("an action occurs with {n}")]
fn action_with_num(n: i32) { let _ = n; }

#[then("only fast examples run")]
fn only_fast_examples_run(num: &'static str) {
    assert_eq!(num, "1", "unexpected example row executed");
}

#[then("events are recorded")]
fn events_recorded() {}

#[given("a scenario completes successfully")]
fn scenario_completes_successfully() {}

/// Assert the runtime contract from inside an executed generated scenario:
/// the reporting collector holds a record for the executed scenario whose
/// `feature_path` is exactly the expected manifest-relative path, and the
/// harness metadata carries the same value.
///
/// The report guard lives in the generated runner closure, so by the time
/// `run_without_context` returns the collector holds this execution's record.
/// `snapshot` (not `drain`) leaves concurrently executing scenarios' records
/// untouched; the exact scenario-name match makes extra records harmless.
fn assert_runtime_feature_path<T>(
    request: StdScenarioRunRequest<'_, T>,
    expected_feature_path: &str,
) -> T {
    let scenario_name = request.metadata().scenario_name().to_owned();
    let observed_metadata_path = request.metadata().feature_path().to_owned();
    let outcome = request.run_without_context();

    let Some(record) = reporting::snapshot()
        .into_iter()
        .find(|record| record.scenario_name() == scenario_name)
    else {
        panic!("the executed scenario `{scenario_name}` must be recorded in the collector");
    };
    assert_eq!(
        record.feature_path(),
        expected_feature_path,
        "the recorded feature path must be manifest-relative with / separators"
    );
    assert_eq!(
        observed_metadata_path, expected_feature_path,
        "the harness metadata must carry the same manifest-relative path"
    );
    outcome
}

// ---------------------------------------------------------------------------
// Route 1: `#[scenario]` binds a crate-local feature file.
// ---------------------------------------------------------------------------

/// Harness that delegates the runtime assertion for the `#[scenario]` route.
#[derive(Default)]
struct ScenarioMetadataCaptureHarness;

impl HarnessAdapter for ScenarioMetadataCaptureHarness {
    type Context = ();

    fn run<T>(&self, request: StdScenarioRunRequest<'_, T>) -> Result<T, HarnessError> {
        Ok(assert_runtime_feature_path(
            request,
            "tests/features/reporting.feature",
        ))
    }
}

#[scenario(
    path = "tests/features/reporting.feature",
    harness = ScenarioMetadataCaptureHarness,
)]
// The `#[scenario]` route records the manifest-relative feature path in the
// reporting collector when the generated test executes. The assertions run
// inside `ScenarioMetadataCaptureHarness`, which the generated test delegates
// to; the runner's own scheduling of this test is the execution that produces
// the runtime record.
fn scenario_records_manifest_relative_path() {}

// ---------------------------------------------------------------------------
// Route 2: `scenarios!` binds a crate-local feature directory.
// ---------------------------------------------------------------------------

/// Harness that delegates the runtime assertion for the `scenarios!` route.
///
/// rstest's generated wrapper does not preserve the function's visibility, so
/// a `scenarios!`-generated test cannot be called by path from the enclosing
/// module; the assertions therefore run in the harness adapter the generated
/// test delegates to, the same pattern as `scenarios_macro.rs`.
#[derive(Default)]
struct ScenariosMetadataCaptureHarness;

impl HarnessAdapter for ScenariosMetadataCaptureHarness {
    type Context = ();

    fn run<T>(&self, request: StdScenarioRunRequest<'_, T>) -> Result<T, HarnessError> {
        let expected_feature_path = match request.metadata().scenario_name() {
            "fast macro scenario" => "tests/features/filtered/fast.feature",
            "outline example" => "tests/features/filtered/mixed.feature",
            name => panic!("unexpected filtered scenario metadata: {name}"),
        };
        Ok(assert_runtime_feature_path(request, expected_feature_path))
    }
}

scenarios!(
    "tests/features/filtered",
    tags = "@fast",
    harness = ScenariosMetadataCaptureHarness,
);
// The `scenarios!` route records the manifest-relative feature path when a
// generated test executes. The `tags = "@fast"` filter generates tests only
// for the `@fast` scenarios in the bound directory; the `@slow` scenarios
// generate none, which is exactly the case a body-scoped tracking binding
// would miss. The executable regression tests are the `scenarios!`-generated
// tests in `filtered_scenarios` (`fast_fast_macro_scenario` and
// `mixed_outline_example::case_1`): each delegates to
// `ScenariosMetadataCaptureHarness`, which reads the `ScenarioRecord` the
// executed generated code recorded and asserts the exact manifest-relative
// path with `/` separators.
