//! Helpers for manipulating the shared running total fixture used across behavioural tests.

use std::cell::RefCell;

// Reset the accumulator so each scenario starts from a predictable baseline.
pub fn set_total(total: &RefCell<i32>, value: i32) {
    *total.borrow_mut() = value;
}

// Apply an additional value contributed by a translated step.
pub fn add_to_total(total: &RefCell<i32>, value: i32) {
    *total.borrow_mut() += value;
}

// Assert that the accumulator equals the expected sum once a scenario completes.
pub fn assert_total(total: &RefCell<i32>, expected: i32) {
    assert_eq!(*total.borrow(), expected);
}

// Assert that the accumulator does not match a value ruled out by the scenario.
pub fn assert_total_not(total: &RefCell<i32>, forbidden: i32) {
    assert_ne!(*total.borrow(), forbidden);
}
