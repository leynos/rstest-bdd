//! Behavioural tests covering multi-language Gherkin parsing.
use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

/// Mutable accumulator shared by all multilingual scenarios.
/// Using `RefCell` avoids borrowing conflicts between steps while keeping
/// the fixture synchronous and lightweight.
#[fixture]
fn running_total() -> RefCell<i32> {
    RefCell::new(0)
}

/// Reset the accumulator to match the language-agnostic starting step.
fn set_total(total: &RefCell<i32>, value: i32) {
    *total.borrow_mut() = value;
}

/// Accumulate an additional value supplied by any translated `And` step.
fn add_to_total(total: &RefCell<i32>, value: i32) {
    *total.borrow_mut() += value;
}

/// Verify the running total matches the Then step's expectation.
fn assert_total(total: &RefCell<i32>, expected: i32) {
    assert_eq!(*total.borrow(), expected);
}

/// Guard against incorrect totals when the scenario uses a But step.
fn assert_total_not(total: &RefCell<i32>, forbidden: i32) {
    assert_ne!(*total.borrow(), forbidden);
}

#[given("the starting value is {value:i32}")]
fn starting_value(running_total: &RefCell<i32>, value: i32) {
    set_total(running_total, value);
}

#[given("an additional value is {value:i32}")]
fn additional_value(running_total: &RefCell<i32>, value: i32) {
    add_to_total(running_total, value);
}

#[when("I add {value:i32}")]
fn add_value(running_total: &RefCell<i32>, value: i32) {
    add_to_total(running_total, value);
}

#[then("the total is {expected:i32}")]
fn total_is(running_total: &RefCell<i32>, expected: i32) {
    assert_total(running_total, expected);
}

#[then("the total is not {forbidden:i32}")]
fn total_is_not(running_total: &RefCell<i32>, forbidden: i32) {
    assert_total_not(running_total, forbidden);
}

#[scenario(path = "tests/features/i18n/addition_fr.feature")]
fn addition_in_french(running_total: RefCell<i32>) {
    let _ = running_total;
}

#[scenario(path = "tests/features/i18n/addition_de.feature")]
fn addition_in_german(running_total: RefCell<i32>) {
    let _ = running_total;
}

#[scenario(path = "tests/features/i18n/addition_es.feature")]
fn addition_in_spanish(running_total: RefCell<i32>) {
    let _ = running_total;
}

#[scenario(path = "tests/features/i18n/addition_ru.feature")]
fn addition_in_russian(running_total: RefCell<i32>) {
    let _ = running_total;
}

#[scenario(path = "tests/features/i18n/addition_ja.feature")]
fn addition_in_japanese(running_total: RefCell<i32>) {
    let _ = running_total;
}

#[scenario(path = "tests/features/i18n/addition_ar.feature")]
fn addition_in_arabic(running_total: RefCell<i32>) {
    let _ = running_total;
}
