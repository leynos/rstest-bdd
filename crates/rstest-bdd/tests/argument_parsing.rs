//! Behavioural test for step argument parsing

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;

#[fixture]
fn account() -> RefCell<u32> {
    RefCell::new(0)
}

#[given("I start with {amount:u32} dollars")]
fn start_balance(account: &RefCell<u32>, amount: u32) {
    *account.borrow_mut() = amount;
}

#[when("I deposit {amount:u32} dollars")]
fn deposit_amount(account: &RefCell<u32>, amount: u32) {
    *account.borrow_mut() += amount;
}

#[then("my balance is {expected:u32} dollars")]
fn check_balance(account: &RefCell<u32>, expected: u32) {
    assert_eq!(*account.borrow(), expected);
}

#[scenario(path = "tests/features/argument.feature")]
fn deposit_scenario(account: RefCell<u32>) {
    let _ = account;
}
