use rstest::fixture;
use rstest_bdd::ScenarioState as ScenarioStateTrait;
use rstest_bdd::state::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};

#[derive(ScenarioState)]
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

#[when("I record the value {value:int}")]
fn record_value(cart_state: &CartState, value: i32) {
    cart_state.total.set(value);
    assert!(cart_state.total.is_filled());
}

#[when("I clear the cart state")]
fn clear_state(cart_state: &CartState) {
    ScenarioStateTrait::reset(cart_state);
}

#[then("the recorded value is {expected:int}")]
fn check_value(cart_state: &CartState, expected: i32) {
    assert_eq!(cart_state.total.get(), Some(expected));
}

#[then("no value is stored")]
fn no_value(cart_state: &CartState) {
    assert!(cart_state.total.is_empty());
}

#[scenario(path = "tests/features/scenario_state.feature", index = 0)]
fn scenario_preserves_value(cart_state: CartState) {
    let _ = cart_state;
}

#[scenario(path = "tests/features/scenario_state.feature", index = 1)]
fn scenario_clears_value(cart_state: CartState) {
    let _ = cart_state;
}
