//! BDD acceptance tests for the GPUI counter example.
//!
//! These tests demonstrate `GpuiHarness` and `GpuiAttributePolicy` in a
//! user-facing example crate, with step access to injected
//! `gpui::TestAppContext` via the `rstest_bdd_harness_context` fixture key.

use gpui_counter::CounterApp;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[fixture]
fn app() -> CounterApp {
    CounterApp::new(0)
}

#[given("a counter starting at {start:i32}")]
fn a_counter_starting_at(app: &CounterApp, start: i32) {
    // Re-initialise the counter to the requested starting value.
    app.increment(start.saturating_sub(app.value()));
}

#[when("I increment the counter by {amount:i32}")]
fn increment_counter(app: &CounterApp, amount: i32) {
    app.increment(amount);
}

#[when("I decrement the counter by {amount:i32}")]
fn decrement_counter(app: &CounterApp, amount: i32) {
    app.decrement(amount);
}

#[when("I record the GPUI dispatcher seed")]
fn record_dispatcher_seed(
    app: &CounterApp,
    #[from(rstest_bdd_harness_context)] context: &gpui::TestAppContext,
) {
    app.record_dispatcher_seed(context.dispatcher().seed());
}

#[then("the counter value is {expected:i32}")]
fn counter_value_is(app: &CounterApp, expected: i32) {
    assert_eq!(app.value(), expected);
}

#[then("the recorded dispatcher seed is {expected:u64}")]
fn recorded_dispatcher_seed_is(app: &CounterApp, expected: u64) {
    assert_eq!(app.dispatcher_seed(), Some(expected));
}

#[scenario(
    path = "tests/features/counter.feature",
    name = "Increment a counter and observe GPUI context",
    harness = rstest_bdd_harness_gpui::GpuiHarness,
    attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy,
)]
fn increment_and_observe_gpui_context(#[from(app)] _: CounterApp) {}

#[scenario(
    path = "tests/features/counter.feature",
    name = "Multiple increments and decrements",
    harness = rstest_bdd_harness_gpui::GpuiHarness,
    attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy,
)]
fn multiple_increments_and_decrements(#[from(app)] _: CounterApp) {}
