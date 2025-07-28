//! End-to-end test verifying fixture injection across multiple steps

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[fixture]
fn number() -> i32 {
    21
}

#[fixture]
fn multiplier() -> i32 {
    2
}

#[given("number is available")]
fn check_number(#[from(number)] n: i32) {
    assert_eq!(n, 21);
}

#[when("the number is doubled")]
fn multiply(#[from(number)] n: i32, #[from(multiplier)] m: i32) {
    assert_eq!(n * m, 42);
}

#[then("the number remains")]
fn verify_number(#[from(number)] n: i32) {
    assert_eq!(n, 21);
}

#[scenario(path = "tests/features/context.feature")]
fn scenario_steps(number: i32, multiplier: i32) {
    // The parameters are unused here but must be present so the macro can
    // insert these fixtures into the `StepContext` for each step.
    let _ = (number, multiplier);
}
