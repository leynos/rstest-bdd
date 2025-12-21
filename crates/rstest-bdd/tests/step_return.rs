//! Behavioural test verifying step return value injection

use rstest::fixture;
use rstest_bdd::StepResult;
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
    reason = "step intentionally returns Result to exercise wrapper normalisation"
)]
fn fallible_unit_step_succeeds(number: Number) -> Result<(), &'static str> {
    assert_eq!(number.0, 1);
    Ok(())
}

#[when("a fallible increment succeeds")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step intentionally returns Result to exercise wrapper normalisation"
)]
fn fallible_increment_succeeds(number: Number) -> Result<Number, &'static str> {
    Ok(Number(number.0 + 1))
}

#[when("a fallible increment fails")]
fn fallible_increment_fails(number: Number) -> Result<Number, &'static str> {
    assert_eq!(number.0, 1);
    Err("value failure")
}

#[when("a std fallible unit step succeeds")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step intentionally returns Result to exercise wrapper normalisation"
)]
fn std_fallible_unit_step_succeeds(number: Number) -> std::result::Result<(), &'static str> {
    assert_eq!(number.0, 1);
    Ok(())
}

#[when("a core fallible increment succeeds")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step intentionally returns Result to exercise wrapper normalisation"
)]
fn core_fallible_increment_succeeds(number: Number) -> core::result::Result<Number, &'static str> {
    Ok(Number(number.0 + 1))
}

#[when("a StepResult increment succeeds")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step intentionally returns Result to exercise wrapper normalisation"
)]
fn stepresult_increment_succeeds(number: Number) -> StepResult<Number, &'static str> {
    Ok(Number(number.0 + 1))
}

type AliasResult<T> = Result<T, &'static str>;

#[when(result)]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step intentionally returns a Result alias to exercise return-kind override"
)]
fn alias_increment_succeeds(number: Number) -> AliasResult<Number> {
    assert_eq!(number.0, 1);
    Ok(Number(number.0 + 1))
}

#[when(result)]
fn alias_increment_fails(number: Number) -> AliasResult<Number> {
    assert_eq!(number.0, 1);
    Err("alias failure")
}

#[when(value)]
fn value_increment_succeeds(number: Number) -> Number {
    assert_eq!(number.0, 1);
    Number(number.0 + 1)
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

#[then("the fallible result fails")]
fn fallible_result_fails() -> Result<(), &'static str> {
    Err("fallible failure scenario should stop before assertions")
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

#[scenario(path = "tests/features/step_return_fallible_result_success.feature")]
fn scenario_fallible_result_success(number: Number) {
    let _ = number;
}

#[scenario(path = "tests/features/step_return_fallible_result_failure.feature")]
#[should_panic(expected = "value failure")]
fn scenario_fallible_result_failure(number: Number) {
    let _ = number;
}

#[scenario(path = "tests/features/step_return_std_result.feature")]
fn scenario_std_result(number: Number) {
    let _ = number;
}

#[scenario(path = "tests/features/step_return_core_result.feature")]
fn scenario_core_result(number: Number) {
    let _ = number;
}

#[scenario(path = "tests/features/step_return_stepresult.feature")]
fn scenario_stepresult(number: Number) {
    let _ = number;
}

#[scenario(path = "tests/features/step_return_value_override.feature")]
fn scenario_value_override(number: Number) {
    let _ = number;
}

#[scenario(path = "tests/features/step_return_alias_override_success.feature")]
fn scenario_alias_override_success(number: Number) {
    let _ = number;
}

#[scenario(path = "tests/features/step_return_alias_override.feature")]
#[should_panic(expected = "alias failure")]
fn scenario_alias_override_failure(number: Number) {
    let _ = number;
}
