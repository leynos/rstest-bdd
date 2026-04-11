//! Behavioural coverage for async-loop semantics that should stay stable across
//! code generation refactors.

#[path = "common/async_semantic_behaviour_support.rs"]
mod async_semantic_behaviour_support;

use std::cell::RefCell;
use std::panic::catch_unwind;

use rstest::fixture;
use rstest_bdd::assert_scenario_skipped;
use rstest_bdd::panic_message;
use rstest_bdd::reporting::drain as drain_reports;
use rstest_bdd_macros::{given, scenario, then, when};
use serial_test::serial;

#[cfg(feature = "diagnostics")]
use async_semantic_behaviour_support::assert_bypassed_step_recorded;
use async_semantic_behaviour_support::{
    CleanupProbe, ERROR_SCENARIO_NAME, FEATURE_PATH, SKIP_SCENARIO_NAME, SemanticValue,
    assert_feature_path_suffix, assert_handler_failure_context, cleanup_drops, clear_events,
    push_event, reset_cleanup_drops, scenario_line, snapshot_events,
};

#[fixture]
fn semantic_order_fixture() -> RefCell<Vec<String>> {
    RefCell::new(vec!["fixture-created".into()])
}

#[fixture]
fn semantic_value_fixture() -> SemanticValue {
    SemanticValue(1)
}

#[fixture]
fn semantic_shared_counter() -> RefCell<usize> {
    RefCell::new(0)
}

#[fixture]
fn semantic_cleanup_probe() -> CleanupProbe {
    CleanupProbe
}

#[given("semantic async skip state is reset")]
fn semantic_skip_state_reset() {
    clear_events();
    push_event("skip:given");
}

#[when("semantic async skip is requested")]
async fn semantic_skip_requested() {
    push_event("skip:when");
    tokio::task::yield_now().await;
    rstest_bdd::skip!("semantic async skip message");
}

#[then("semantic async trailing step should never run")]
fn semantic_skip_trailing_step() {
    push_event("skip:then");
    panic!("skip propagation failed to stop trailing steps");
}

#[given("semantic async order fixture starts with its creation marker")]
fn semantic_order_fixture_starts_clean(
    #[from(semantic_order_fixture)] order_fixture: &RefCell<Vec<String>>,
) {
    assert_eq!(
        order_fixture.borrow().as_slice(),
        ["fixture-created"],
        "fixture should be available before declaration-order checks run",
    );
    order_fixture.borrow_mut().push("given".into());
}

#[when(expr = "semantic async order fixture records {item:string}")]
fn semantic_order_fixture_records(
    #[from(semantic_order_fixture)] order_fixture: &RefCell<Vec<String>>,
    item: String,
) {
    order_fixture.borrow_mut().push(format!("when:{item}"));
}

#[then(expr = "semantic async order fixture includes {item:string} in sequence")]
fn semantic_order_fixture_includes(
    #[from(semantic_order_fixture)] order_fixture: &RefCell<Vec<String>>,
    item: String,
) {
    order_fixture.borrow_mut().push(format!("then:{item}"));
}

#[given("semantic async base value is 1")]
fn semantic_base_value_is_one(semantic_value_fixture: SemanticValue) {
    assert_eq!(semantic_value_fixture, SemanticValue(1));
}

#[when("semantic async derived value is produced")]
async fn semantic_derived_value_is_produced(
    semantic_value_fixture: SemanticValue,
) -> SemanticValue {
    tokio::task::yield_now().await;
    SemanticValue(semantic_value_fixture.0 + 1)
}

#[then("semantic async next step receives value 2")]
fn semantic_next_step_receives_value(semantic_value_fixture: SemanticValue) {
    assert_eq!(semantic_value_fixture, SemanticValue(2));
}

#[given("semantic async shared counter starts at 0")]
fn semantic_shared_counter_starts_at_zero(semantic_shared_counter: &RefCell<usize>) {
    assert_eq!(*semantic_shared_counter.borrow(), 0);
}

#[when("semantic async shared counter increments")]
async fn semantic_shared_counter_increments(semantic_shared_counter: &RefCell<usize>) {
    {
        let mut counter = semantic_shared_counter.borrow_mut();
        *counter += 1;
    }
    tokio::task::yield_now().await;
}

#[then("semantic async shared counter equals 2")]
fn semantic_shared_counter_equals_two(semantic_shared_counter: &RefCell<usize>) {
    assert_eq!(*semantic_shared_counter.borrow(), 2);
}

#[given("semantic async failure state is reset")]
fn semantic_failure_state_reset() {
    clear_events();
    push_event("failure:given");
}

#[when("semantic async failing step runs")]
async fn semantic_async_failing_step() -> Result<(), &'static str> {
    push_event("failure:when");
    tokio::task::yield_now().await;
    Err("semantic async failure")
}

#[then("semantic async failure trailing step should never run")]
fn semantic_failure_trailing_step() {
    push_event("failure:then");
    panic!("error propagation failed to stop trailing steps");
}

#[given("semantic cleanup probe is available")]
fn semantic_cleanup_probe_is_available(
    #[from(semantic_cleanup_probe)] _semantic_cleanup_probe: &CleanupProbe,
) {
}

#[when("semantic cleanup step fails")]
fn semantic_cleanup_step_fails(
    #[from(semantic_cleanup_probe)] _semantic_cleanup_probe: &CleanupProbe,
) -> Result<(), &'static str> {
    Err("cleanup probe failure")
}

#[scenario(
    path = "tests/features/async_semantic_behaviour.feature",
    name = "async skip propagation preserves metadata"
)]
async fn semantic_async_skip_scenario() {
    panic!("scenario body should not execute after a skip request");
}

#[scenario(
    path = "tests/features/async_semantic_behaviour.feature",
    name = "async steps preserve declaration order"
)]
fn semantic_step_ordering_outline(
    #[from(semantic_order_fixture)] semantic_order_fixture: RefCell<Vec<String>>,
    item: String,
) {
    let expected = vec![
        "fixture-created".to_string(),
        "given".to_string(),
        format!("when:{item}"),
        format!("then:{item}"),
    ];
    assert_eq!(
        semantic_order_fixture.into_inner(),
        expected,
        "steps should execute in declaration order and preserve fixtures across the outline case",
    );
}

#[scenario(
    path = "tests/features/async_semantic_behaviour.feature",
    name = "async returned fixtures reach the next step"
)]
#[tokio::test(flavor = "current_thread")]
async fn semantic_async_returned_fixture_scenario(semantic_value_fixture: SemanticValue) {
    let _ = semantic_value_fixture;
}

#[scenario(
    path = "tests/features/async_semantic_behaviour.feature",
    name = "async RefCell fixtures survive cross-step borrows"
)]
#[tokio::test(flavor = "current_thread")]
async fn semantic_async_refcell_fixture_scenario(
    #[from(semantic_shared_counter)] semantic_shared_counter: RefCell<usize>,
) {
    assert_eq!(
        semantic_shared_counter.into_inner(),
        2,
        "RefCell-backed fixtures should remain borrowable across async step boundaries",
    );
}

#[scenario(
    path = "tests/features/async_semantic_behaviour.feature",
    name = "async failure surfaces scenario metadata"
)]
#[ignore = "exercised by error_propagation_includes_step_and_scenario_context"]
async fn semantic_async_error_scenario() {
    panic!("scenario body should not execute after an earlier failure");
}

#[scenario(
    path = "tests/features/async_semantic_behaviour.feature",
    name = "cleanup probe completes successfully"
)]
fn semantic_cleanup_success_scenario(
    #[from(semantic_cleanup_probe)] _semantic_cleanup_probe: CleanupProbe,
) {
}

#[scenario(
    path = "tests/features/async_semantic_behaviour.feature",
    name = "cleanup probe fails after setup"
)]
#[ignore = "exercised by cleanup_probe_drops_after_failed_scenario_completion"]
fn semantic_cleanup_failure_scenario(
    #[from(semantic_cleanup_probe)] _semantic_cleanup_probe: CleanupProbe,
) {
}

#[test]
#[serial]
fn skip_propagation_preserves_message_and_bypass_metadata() {
    let _ = drain_reports();
    semantic_async_skip_scenario();
    let skip_scenario_line = scenario_line(SKIP_SCENARIO_NAME);

    assert_eq!(
        snapshot_events(),
        vec!["skip:given".to_string(), "skip:when".to_string()],
        "skip propagation should stop later steps from running",
    );

    let records = drain_reports();
    let [record] = records.as_slice() else {
        panic!("expected a single skip record");
    };
    assert_feature_path_suffix(record.feature_path(), FEATURE_PATH);
    assert_eq!(record.scenario_name(), SKIP_SCENARIO_NAME);
    assert_eq!(record.line(), skip_scenario_line);
    let details = assert_scenario_skipped!(
        record.status(),
        message = "semantic async skip message",
        allow_skipped = true,
        forced_failure = false,
    );
    assert_eq!(details.message(), Some("semantic async skip message"));

    #[cfg(feature = "diagnostics")]
    assert_bypassed_step_recorded(
        SKIP_SCENARIO_NAME,
        skip_scenario_line,
        "semantic async trailing step should never run",
        "semantic async skip message",
    );
}

#[test]
fn error_propagation_includes_step_and_scenario_context() {
    let panic = match catch_unwind(semantic_async_error_scenario) {
        Ok(()) => panic!("expected async scenario to panic"),
        Err(panic) => panic,
    };
    let message = panic_message(panic.as_ref());

    assert_handler_failure_context(
        &message,
        FEATURE_PATH,
        ERROR_SCENARIO_NAME,
        "When",
        "semantic async failing step runs",
        "semantic_async_failing_step",
        "semantic async failure",
    );
    assert_eq!(
        snapshot_events(),
        vec!["failure:given".to_string(), "failure:when".to_string()],
        "failure propagation should stop later steps from running",
    );
}

#[test]
fn cleanup_probe_drops_after_successful_scenario_completion() {
    reset_cleanup_drops();
    semantic_cleanup_success_scenario();
    assert_eq!(
        cleanup_drops(),
        1,
        "fixtures should be dropped after successful scenario completion",
    );
}

#[test]
fn cleanup_probe_drops_after_failed_scenario_completion() {
    reset_cleanup_drops();
    let result = catch_unwind(semantic_cleanup_failure_scenario);
    assert!(result.is_err(), "expected cleanup scenario to fail");
    assert_eq!(
        cleanup_drops(),
        1,
        "fixtures should be dropped exactly once even when scenario execution fails",
    );
}
