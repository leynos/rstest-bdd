//! Behavioural tests for the `scenarios!` macro.

use rstest_bdd::StepError;
use rstest_bdd_macros::{given, scenarios, then, when};

#[given("a precondition")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step functions must return StepError"
)]
fn precondition() -> Result<(), StepError> {
    Ok(())
}

#[when("an action occurs")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step functions must return StepError"
)]
fn action() -> Result<(), StepError> {
    Ok(())
}

#[when("an action occurs with <num>")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step functions must return StepError"
)]
fn action_with_num() -> Result<(), StepError> {
    Ok(())
}

#[then("events are recorded")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step functions must return StepError"
)]
fn events_recorded() -> Result<(), StepError> {
    Ok(())
}

scenarios!("tests/features/auto");
