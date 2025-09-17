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
