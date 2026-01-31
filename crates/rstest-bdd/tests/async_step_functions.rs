//! Behavioural tests covering native `async fn` step execution.
//!
//! This suite validates that:
//! - Async scenarios await async step bodies natively (no sync wrapper path)
//! - Sync scenarios can still execute async-only steps via a blocking fallback
//! - Mutable fixtures borrowed from `StepContext` remain valid across `.await`

use rstest::fixture;
use rstest_bdd::{StepContext, StepKeyword, StepText, find_step_with_metadata};
use rstest_bdd_macros::{given, scenario, then, when};

#[derive(Default)]
struct CounterState {
    value: usize,
}

#[fixture]
fn state() -> CounterState {
    CounterState::default()
}

#[given("a counter state starts at 0")]
fn counter_state_starts_at_zero(#[from(state)] state: &mut CounterState) {
    state.value = 0;
}

#[when("an async step increments the state")]
async fn async_step_increments_state(#[from(state)] state: &mut CounterState) {
    let start = state.value;
    tokio::task::yield_now().await;
    state.value = start + 1;
}

#[when("a sync step increments the state")]
fn sync_step_increments_state(#[from(state)] state: &mut CounterState) {
    state.value += 1;
}

#[then(expr = "the state value is {n}")]
fn state_value_is(#[from(state)] state: &CounterState, n: usize) {
    assert_eq!(state.value, n);
}

#[scenario(
    path = "tests/features/async_step_functions.feature",
    name = "Async scenario runs async step bodies"
)]
#[tokio::test(flavor = "current_thread")]
async fn async_scenario_awaits_async_steps(state: CounterState) {
    assert_eq!(state.value, 2);
}

#[scenario(
    path = "tests/features/async_step_functions.feature",
    name = "Sync scenario can execute async steps via blocking fallback"
)]
#[test]
fn sync_scenario_can_block_on_async_steps(state: CounterState) {
    assert_eq!(state.value, 2);
}

#[tokio::test(flavor = "current_thread")]
async fn sync_wrapper_refuses_to_create_nested_runtime() {
    let step = find_step_with_metadata(
        StepKeyword::When,
        StepText::from("an async step increments the state"),
    )
    .unwrap_or_else(|| panic!("expected async step to be registered"));

    let mut ctx = StepContext::default();
    let state_cell = StepContext::owned_cell(CounterState::default());
    ctx.insert_owned::<CounterState>("state", &state_cell);

    let Err(err) = (step.run)(&mut ctx, "an async step increments the state", None, None) else {
        panic!("expected sync wrapper to refuse nested Tokio runtime");
    };

    assert!(
        err.to_string().contains("Tokio runtime"),
        "expected nested runtime diagnostic, got: {err}"
    );
}
