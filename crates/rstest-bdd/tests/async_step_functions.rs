//! Behavioural tests covering native `async fn` step execution.
//!
//! This suite validates that:
//! - Async scenarios await async step bodies natively (no sync wrapper path)
//! - Sync scenarios can still execute async-only steps via a blocking fallback
//! - Mutable fixtures borrowed from `StepContext` remain valid across `.await`

use rstest::fixture;
use rstest_bdd::{
    StepContext, StepCtx, StepDoc, StepExecution, StepFuture, StepKeyword, StepTable, StepText,
    StepTextRef, async_step::sync_to_async, find_step_with_metadata,
};
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
#[expect(
    clippy::expect_used,
    reason = "test asserts that the async step is registered before invoking its sync wrapper"
)]
async fn sync_wrapper_refuses_to_create_nested_runtime() {
    let step = find_step_with_metadata(
        StepKeyword::When,
        StepText::from("an async step increments the state"),
    )
    .expect("expected async step to be registered");

    let mut ctx = StepContext::default();
    let state_cell = StepContext::owned_cell(CounterState::default());
    ctx.insert_owned::<CounterState>("state", &state_cell);

    let Err(err) = (step.run)(&mut ctx, "an async step increments the state", None, None) else {
        panic!("expected sync wrapper to refuse nested Tokio runtime");
    };

    assert!(
        err.to_string().contains("harness-provided runtime"),
        "expected harness-provided runtime diagnostic, got: {err}"
    );
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "helper must match the StepFn signature"
)]
fn manual_sync_step(
    _ctx: &mut StepContext<'_>,
    text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, rstest_bdd::StepError> {
    let payload = format!("wrapped:{text}");
    Ok(StepExecution::from_value(Some(Box::new(payload))))
}

fn manual_async_wrapper<'ctx>(
    ctx: StepCtx<'ctx, '_>,
    text: StepTextRef<'ctx>,
    docstring: StepDoc<'ctx>,
    table: StepTable<'ctx>,
) -> StepFuture<'ctx> {
    sync_to_async(manual_sync_step)(ctx, text, docstring, table)
}

#[tokio::test(flavor = "current_thread")]
#[expect(
    clippy::expect_used,
    reason = "test validates payload downcast from wrapper result"
)]
async fn public_sync_to_async_helper_supports_alias_based_wrapper_signatures() {
    let _: rstest_bdd::AsyncStepFn = manual_async_wrapper;

    let mut ctx = StepContext::default();
    let execution = manual_async_wrapper(&mut ctx, "example", None, None)
        .await
        .expect("wrapped step should execute successfully");

    let StepExecution::Continue {
        value: Some(payload),
    } = execution
    else {
        panic!("expected Continue outcome with payload");
    };
    let value = payload
        .downcast::<String>()
        .expect("payload should be the wrapped sync result");
    assert_eq!(*value, "wrapped:example");
}
