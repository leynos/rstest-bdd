//! Behaviour tests for scenario state slots and reset semantics.

use rstest::fixture;
use rstest_bdd::ScenarioState as _;
use rstest_bdd::Slot;
use rstest_bdd_macros::{given, scenario, then, when, ScenarioState};

#[derive(Default, ScenarioState)]
struct CartState {
    total: Slot<i32>,
}

#[fixture]
fn cart_state() -> CartState {
    CartState::default()
}

#[given("an empty cart state")]
fn empty_state(cart_state: &CartState) {
    assert!(cart_state.total.is_empty());
}

#[when("I record the value {value:i32}")]
fn record_value(cart_state: &CartState, value: i32) {
    cart_state.total.set(value);
    assert!(cart_state.total.is_filled());
}

#[when("I clear the cart state")]
fn clear_state(cart_state: &CartState) {
    cart_state.reset();
}

#[then("the recorded value is {expected:i32}")]
fn check_value(cart_state: &CartState, expected: i32) {
    assert_eq!(cart_state.total.get(), Some(expected));
}

#[then("no value is stored")]
fn no_value(cart_state: &CartState) {
    assert!(cart_state.total.is_empty());
}

#[scenario(
    path = "tests/features/scenario_state.feature",
    name = "Recording a single value"
)]
fn scenario_preserves_value(cart_state: CartState) {
    let _ = cart_state;
}

#[scenario(
    path = "tests/features/scenario_state.feature",
    name = "Clearing stored values"
)]
fn scenario_clears_value(cart_state: CartState) {
    let _ = cart_state;
}
