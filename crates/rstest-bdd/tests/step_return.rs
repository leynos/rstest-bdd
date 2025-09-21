//! Behavioural test verifying step return value injection

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Number(i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PrimaryValue(i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SecondaryValue(i32);

#[fixture]
fn number() -> Number {
    Number(1)
}

#[given("base number is 1")]
fn base(number: Number) {
    assert_eq!(number.0, 1);
}

#[when("it is incremented")]
fn increment(number: Number) -> Number {
    Number(number.0 + 1)
}

#[when("a fallible unit step succeeds")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step intentionally returns Result to exercise IntoStepResult"
)]
fn fallible_unit_step_succeeds(number: Number) -> Result<(), &'static str> {
    assert_eq!(number.0, 1);
    Ok(())
}

#[when("a fallible increment succeeds")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step intentionally returns Result to exercise IntoStepResult"
)]
fn fallible_increment_succeeds(number: Number) -> Result<Number, &'static str> {
    Ok(Number(number.0 + 1))
}

#[then("the result is 2")]
fn check(number: Number) {
    assert_eq!(number.0, 2);
}

#[then("the base number is unchanged")]
fn base_number_unchanged(number: Number) {
    assert_eq!(number.0, 1);
}

#[then("the fallible result is 2")]
fn fallible_result_is_two(number: Number) {
    assert_eq!(number.0, 2);
}

#[scenario(path = "tests/features/step_return.feature")]
fn scenario_step_return(number: Number) {
    let _ = number;
}

#[fixture]
fn primary_value() -> PrimaryValue {
    PrimaryValue(10)
}

#[fixture]
fn competing_primary_value() -> PrimaryValue {
    PrimaryValue(15)
}

#[fixture]
fn secondary_value() -> SecondaryValue {
    SecondaryValue(20)
}

#[given("two competing fixtures")]
fn two_competing(
    primary_value: PrimaryValue,
    competing_primary_value: PrimaryValue,
    secondary_value: SecondaryValue,
) {
    assert_eq!(primary_value.0, 10);
    assert_eq!(competing_primary_value.0, 15);
    assert_eq!(secondary_value.0, 20);
}

#[when("a step returns a competing value")]
fn returns_competing_value(
    primary_value: PrimaryValue,
    secondary_value: SecondaryValue,
) -> PrimaryValue {
    // When multiple fixtures of the same type exist, the step context cannot
    // determine which fixture to override, so the return value is ignored.
    // The `competing_primary_value` fixture creates this ambiguity intentionally.
    PrimaryValue(primary_value.0 + secondary_value.0)
}

#[then("the fixtures remain unchanged")]
fn fixtures_remain(
    primary_value: PrimaryValue,
    competing_primary_value: PrimaryValue,
    secondary_value: SecondaryValue,
) {
    assert_eq!(primary_value.0, 10);
    assert_eq!(competing_primary_value.0, 15);
    assert_eq!(secondary_value.0, 20);
}

#[scenario(path = "tests/features/step_return_ambiguous.feature")]
fn scenario_step_return_ambiguous(
    primary_value: PrimaryValue,
    competing_primary_value: PrimaryValue,
    secondary_value: SecondaryValue,
) {
    let _ = (primary_value, competing_primary_value, secondary_value);
}

#[scenario(path = "tests/features/step_return_fallible_unit.feature")]
fn scenario_fallible_unit(number: Number) {
    let _ = number;
}

#[scenario(path = "tests/features/step_return_fallible_result.feature")]
fn scenario_fallible_result(number: Number) {
    let _ = number;
}
