//! End-to-end test for underscore-prefixed scenario parameters.

#![expect(
    clippy::expect_used,
    reason = "test should fail loudly when scenario state is missing"
)]

use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};

#[derive(ScenarioState, Default)]
struct TestState {
    value: Slot<i32>,
}

#[fixture]
fn state() -> TestState {
    TestState::default()
}

#[given("a value of {value:i32}")]
fn given_value(state: &mut TestState, value: i32) {
    state.value.set(value);
}

#[when("the value is doubled")]
fn when_doubled(state: &mut TestState) {
    let value = state
        .value
        .get()
        .expect("value must be set before doubling");
    state.value.set(value * 2);
}

#[then("the value is {expected:i32}")]
fn then_value_is(state: &TestState, expected: i32) {
    assert_eq!(state.value.get(), Some(expected));
}

#[scenario(path = "tests/features/underscore_param.feature")]
fn underscore_param_scenario(_state: TestState) {}
