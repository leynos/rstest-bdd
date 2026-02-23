//! Behavioural tests for Tokio harness adapter execution semantics.

use rstest::{fixture, rstest};
use rstest_bdd_harness::{HarnessAdapter, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner};
use rstest_bdd_harness_tokio::TokioHarness;
use std::cell::Cell;
use std::rc::Rc;

#[fixture]
fn default_metadata() -> ScenarioMetadata {
    ScenarioMetadata::default()
}

#[rstest]
fn tokio_harness_executes_runner_once(default_metadata: ScenarioMetadata) {
    let call_count = Rc::new(Cell::new(0u8));
    let call_count_clone = Rc::clone(&call_count);
    let request = ScenarioRunRequest::new(
        default_metadata,
        ScenarioRunner::new(move || {
            call_count_clone.set(call_count_clone.get() + 1);
            "done"
        }),
    );

    let harness = TokioHarness::new();
    assert_eq!(harness.run(request), "done");
    assert_eq!(call_count.get(), 1);
}

#[rstest]
fn tokio_harness_supports_non_static_runner_borrows(default_metadata: ScenarioMetadata) {
    let mut counter = 0u8;
    let request = ScenarioRunRequest::new(
        default_metadata,
        ScenarioRunner::new(|| {
            counter += 1;
            counter
        }),
    );

    let harness = TokioHarness::new();
    assert_eq!(harness.run(request), 1);
    assert_eq!(counter, 1);
}

/// Verify that a Tokio runtime is genuinely active inside the harness.
#[test]
fn tokio_runtime_is_active_inside_harness() {
    let request = ScenarioRunRequest::new(
        ScenarioMetadata::default(),
        ScenarioRunner::new(|| {
            // Panics if no Tokio runtime is active on the current thread.
            let _handle = tokio::runtime::Handle::current();
            true
        }),
    );
    let harness = TokioHarness::new();
    assert!(harness.run(request));
}

/// Verify that `spawn_local` is available inside the harness, confirming that
/// a `LocalSet` + `current_thread` runtime is active.
#[rstest]
fn tokio_harness_supports_spawn_local(default_metadata: ScenarioMetadata) {
    let flag = Rc::new(Cell::new(false));
    let flag_clone = Rc::clone(&flag);
    let request = ScenarioRunRequest::new(
        default_metadata,
        ScenarioRunner::new(move || {
            // `spawn_local` panics if no `LocalSet` context is active.
            // Successfully calling it proves the current-thread + LocalSet
            // wiring provided by TokioHarness.
            tokio::task::spawn_local(async move {
                flag_clone.set(true);
            });
            "spawned"
        }),
    );

    let harness = TokioHarness::new();
    assert_eq!(harness.run(request), "spawned");
    // The LocalSet drives the spawned task to completion before block_on
    // returns, so the flag should be set.
    assert!(flag.get(), "spawn_local task should have run to completion");
}

/// Verify that the harness can inspect metadata before executing the runner.
#[test]
fn tokio_harness_passes_metadata_through() {
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
    let harness = TokioHarness::new();
    assert_eq!(harness.run(request), 200);
}
