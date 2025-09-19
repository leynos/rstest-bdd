//! Behavioural tests covering multi-language Gherkin parsing.
#[path = "common/running_total.rs"]
mod running_total_helpers;

use std::cell::RefCell;

use rstest::fixture;
use running_total_helpers::{add_to_total, assert_total, assert_total_not, set_total};
// Import from the macros crate because re-exporting from `rstest_bdd`
// would create a dependency cycle.
use rstest_bdd_macros::{given, scenario, then, when};

/// Mutable accumulator shared by all multilingual scenarios.
/// Using `RefCell` avoids borrowing conflicts between steps while keeping
/// the fixture synchronous and lightweight.
#[fixture]
// Keep the fixture body on one line per review feedback while avoiding
// the `unused_braces` lint via an explicit `return` and preserving the
// inline layout with #[rustfmt::skip].
#[rustfmt::skip]
fn running_total() -> RefCell<i32> { return RefCell::new(0); }

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

macro_rules! i18n_scenario {
    ($name:ident, $path:literal) => {
        #[scenario(path = $path)]
        // Keep the fixture binding so the scenario macro injects the fixture.
        fn $name(running_total: RefCell<i32>) {
            // Drop the fixture binding after injection; steps borrow the shared accumulator.
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
