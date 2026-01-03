//! Integration tests for scenario outline placeholder substitution.
//!
//! These tests verify that `<placeholder>` tokens in step text are substituted
//! with values from the Examples table before step matching occurs.

#![expect(
    clippy::expect_used,
    reason = "integration tests use expect for clarity"
)]

use rstest_bdd_macros::{given, scenario, then, when};
use serial_test::serial;
use std::sync::{LazyLock, Mutex, MutexGuard};

/// Tracks the current item count for arithmetic tests.
static COUNT: LazyLock<Mutex<i32>> = LazyLock::new(|| Mutex::new(0));

fn get_count_guard() -> MutexGuard<'static, i32> {
    match COUNT.lock() {
        Ok(g) => g,
        Err(p) => {
            // Tests intentionally recover after poisoning to keep isolation between scenarios.
            p.into_inner()
        }
    }
}

fn set_count(value: i32) {
    let mut g = get_count_guard();
    *g = value;
}

fn add_count(value: i32) {
    let mut g = get_count_guard();
    *g += value;
}

fn get_count() -> i32 {
    *get_count_guard()
}

/// Step definition using `{n}` capture syntax to extract the substituted value.
#[given("I have {n} items")]
fn have_items(n: i32) {
    set_count(n);
}

/// Step definition using `{n}` capture syntax for the amount to add.
#[when("I add {n} more items")]
fn add_items(n: i32) {
    add_count(n);
}

/// Step definition that verifies the total matches the expected value.
#[then("I should have {n} items")]
fn check_total(n: i32) {
    let actual = get_count();
    assert_eq!(actual, n, "Expected {n} items but found {actual}");
}

/// Test that placeholder substitution works for scenario outlines.
///
/// The feature file contains:
/// - `Given I have <start> items`
/// - `When I add <amount> more items`
/// - `Then I should have <total> items`
///
/// With Examples:
/// - row 1: start=5, amount=3, total=8
/// - row 2: start=10, amount=5, total=15
///
/// The substituted step text should match the `{n}` capture patterns in our
/// step definitions, extracting the actual numeric values.
#[scenario(path = "tests/features/outline_placeholder.feature")]
#[serial]
fn values_are_substituted(start: String, amount: String, total: String) {
    // Verify that the test function received the correct parameter values
    let start_val: i32 = start.parse().expect("start should be a number");
    let amount_val: i32 = amount.parse().expect("amount should be a number");
    let total_val: i32 = total.parse().expect("total should be a number");

    // Verify the arithmetic is correct
    assert_eq!(
        start_val + amount_val,
        total_val,
        "Arithmetic check: {start_val} + {amount_val} should equal {total_val}"
    );
}
