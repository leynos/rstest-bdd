//! Behavioural tests covering multi-language Gherkin parsing.
use std::cell::RefCell;

use rstest::fixture;
// Import from the macros crate because re-exporting from `rstest_bdd`
// would create a dependency cycle.
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
    assert_eq!(*running_total.borrow(), expected);
}

#[then("the total is not {forbidden:i32}")]
fn total_is_not(running_total: &RefCell<i32>, forbidden: i32) {
    assert_ne!(*running_total.borrow(), forbidden);
}

macro_rules! i18n_scenario {
    ($name:ident, $path:literal) => {
        #[scenario(path = $path)]
        fn $name(running_total: RefCell<i32>) {
            // Keep the fixture name so the scenario macro resolves it.
            let _ = running_total;
        }
    };
}

i18n_scenario!(
    addition_in_french,
    "tests/features/i18n/addition_fr.feature"
);
i18n_scenario!(
    addition_in_german,
    "tests/features/i18n/addition_de.feature"
);
i18n_scenario!(
    addition_in_spanish,
    "tests/features/i18n/addition_es.feature"
);
i18n_scenario!(
    addition_in_russian,
    "tests/features/i18n/addition_ru.feature"
);
i18n_scenario!(
    addition_in_japanese,
    "tests/features/i18n/addition_ja.feature"
);
i18n_scenario!(
    addition_in_arabic,
    "tests/features/i18n/addition_ar.feature"
);
