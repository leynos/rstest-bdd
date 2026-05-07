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
    app.set_value(start);
}

#[when("I increment the counter by {amount:u32}")]
fn increment_counter(app: &CounterApp, amount: u32) {
    app.increment(amount);
}

#[when("I decrement the counter by {amount:u32}")]
fn decrement_counter(app: &CounterApp, amount: u32) {
    app.decrement(amount);
}

#[when("I record the GPUI test context")]
fn record_gpui_test_context(
    app: &CounterApp,
    #[from(rstest_bdd_harness_context)] context: &gpui::TestAppContext,
) {
    // NOTE: The GPUI TestAppContext currently returns `None` for
    // `test_function_name()` in this scenario. We call it here to ensure the
    // API is wired correctly without depending on that specific value. If
    // upstream starts populating this field, consider whether the test should
    // start validating or recording the function name instead of ignoring it.
    let _ = context.test_function_name();

    app.record_gpui_context();
}

#[then("the counter value is {expected:i32}")]
fn counter_value_is(app: &CounterApp, expected: i32) {
    assert_eq!(app.value(), expected);
}

#[then("a GPUI test context was recorded")]
fn a_gpui_test_context_was_recorded(app: &CounterApp) {
    assert!(
        app.has_observed_gpui_context(),
        "expected the GPUI test context to have been recorded"
    );
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
