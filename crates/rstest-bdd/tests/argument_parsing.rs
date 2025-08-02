//! Behavioural test for step argument parsing

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;

#[fixture]
fn account() -> RefCell<u32> {
    RefCell::new(0)
}

#[given("I start with {amount:u32} dollars")]
fn start_balance(#[from(account)] acc: &RefCell<u32>, amount: u32) {
    *acc.borrow_mut() = amount;
}

#[when("I deposit {amount:u32} dollars")]
fn deposit_amount(#[from(account)] acc: &RefCell<u32>, amount: u32) {
    *acc.borrow_mut() += amount;
}

#[then("my balance is {expected:u32} dollars")]
fn check_balance(#[from(account)] acc: &RefCell<u32>, expected: u32) {
    assert_eq!(*acc.borrow(), expected);
}

#[scenario(path = "tests/features/argument.feature")]
fn deposit_scenario(account: RefCell<u32>) {
    let _ = account;
}

#[scenario(path = "tests/features/argument_invalid.feature")]
#[should_panic(expected = "failed to parse placeholder 'amount' with value '4294967296' as u32")]
fn deposit_scenario_invalid(account: RefCell<u32>) {
    let _ = account;
}
