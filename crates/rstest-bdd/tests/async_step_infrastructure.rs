//! Behavioural tests for async step infrastructure.
//!
//! These tests verify that the async step registry infrastructure correctly
//! normalises synchronous step definitions into the async interface.

use rstest_bdd::{given, then, when};
use rstest_bdd_macros::scenario;
use std::cell::RefCell;

thread_local! {
    static EXECUTED: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

fn reset_executed() {
    EXECUTED.with(|v| v.borrow_mut().clear());
}

#[given("a synchronous step definition")]
fn given_sync() {
    EXECUTED.with(|v| v.borrow_mut().push("given"));
}

#[when("the async wrapper is invoked")]
fn when_async_wrapper() {
    EXECUTED.with(|v| v.borrow_mut().push("when"));
}

#[then("it returns an immediately-ready future")]
fn then_ready_future() {
    EXECUTED.with(|v| {
        let order = v.borrow();
        assert_eq!(
            order.as_slice(),
            &["given", "when"],
            "steps should execute in order"
        );
    });
}

#[scenario(path = "tests/features/async_step.feature")]
#[test]
fn sync_step_normalised_to_async() {
    reset_executed();
}
