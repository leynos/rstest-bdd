//! Behavioural test verifying step return value injection

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[fixture]
fn number() -> i32 {
    1
}

#[given("base number is 1")]
fn base(number: i32) {
    assert_eq!(number, 1);
}

#[when("it is incremented")]
fn increment(number: i32) -> i32 {
    number + 1
}

#[then("the result is 2")]
fn check(number: i32) {
    assert_eq!(number, 2);
}

#[scenario(path = "tests/features/step_return.feature")]
fn scenario_step_return(number: i32) {
    let _ = number;
}

#[fixture]
fn primary_value() -> i32 {
    10
}

#[fixture]
fn secondary_value() -> i32 {
    20
}

#[given("two competing fixtures")]
fn two_competing(primary_value: i32, secondary_value: i32) {
    assert_eq!(primary_value, 10);
    assert_eq!(secondary_value, 20);
}

#[when("a step returns a competing value")]
fn returns_competing_value(primary_value: i32, secondary_value: i32) -> i32 {
    primary_value + secondary_value
}

#[then("the fixtures remain unchanged")]
fn fixtures_remain(primary_value: i32, secondary_value: i32) {
    assert_eq!(primary_value, 10);
    assert_eq!(secondary_value, 20);
}

#[scenario(path = "tests/features/step_return_ambiguous.feature")]
fn scenario_step_return_ambiguous(primary_value: i32, secondary_value: i32) {
    let _ = (primary_value, secondary_value);
}
