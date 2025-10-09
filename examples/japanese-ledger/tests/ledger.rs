//! BDD acceptance tests for the household ledger using Japanese-language
//! Gherkin scenarios.
//!
//! These tests demonstrate internationalised step definitions and fixture
//! injection with `rstest-bdd`.

use japanese_ledger::HouseholdLedger;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[fixture]
fn ledger() -> HouseholdLedger {
    HouseholdLedger::new()
}

#[given("残高は{start:i32}である")]
fn starting_balance(ledger: &HouseholdLedger, start: i32) {
    ledger.set_balance(start);
}

#[when("残高に{income:i32}を加える")]
fn apply_income(ledger: &HouseholdLedger, income: i32) {
    ledger.apply_income(income);
}

#[when("残高から{expense:i32}を引く")]
fn apply_expense(ledger: &HouseholdLedger, expense: i32) {
    ledger.apply_expense(expense);
}

#[then("残高は{expected:i32}である")]
fn assert_balance(ledger: &HouseholdLedger, expected: i32) {
    assert_eq!(ledger.balance(), expected);
}

#[then("残高は{forbidden:i32}ではない")]
fn assert_balance_not(ledger: &HouseholdLedger, forbidden: i32) {
    assert_ne!(ledger.balance(), forbidden);
}

#[scenario(
    path = "tests/features/household_ledger.feature",
    name = "収入を記録する"
)]
fn records_income(#[from(ledger)] _: HouseholdLedger) {}

#[scenario(
    path = "tests/features/household_ledger.feature",
    name = "支出を記録する"
)]
fn records_expense(#[from(ledger)] _: HouseholdLedger) {}
