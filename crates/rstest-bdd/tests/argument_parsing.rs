//! Behavioural test for step argument parsing

use rstest::fixture;
use rstest_bdd::StepError;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;

#[fixture]
fn account() -> RefCell<u32> {
    RefCell::new(0)
}

#[given("I start with {amount:u32} dollars")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step functions must return StepError"
)]
fn start_balance(#[from(account)] acc: &RefCell<u32>, amount: u32) -> Result<(), StepError> {
    *acc.borrow_mut() = amount;
    Ok(())
}

#[when("I deposit {amount:u32} dollars")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step functions must return StepError"
)]
fn deposit_amount(#[from(account)] acc: &RefCell<u32>, amount: u32) -> Result<(), StepError> {
    *acc.borrow_mut() += amount;
    Ok(())
}

#[then("my balance is {expected:u32} dollars")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step functions must return StepError"
)]
fn check_balance(#[from(account)] acc: &RefCell<u32>, expected: u32) -> Result<(), StepError> {
    assert_eq!(*acc.borrow(), expected);
    Ok(())
}

#[scenario(path = "tests/features/argument.feature")]
fn deposit_scenario(account: RefCell<u32>) {
    let _ = account;
}
