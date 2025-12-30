//! Tests for async scenario execution using `scenarios!` macro with Tokio runtime.
//!
//! This test verifies that:
//! - The `runtime = "tokio-current-thread"` argument generates async tests
//! - Async steps execute sequentially and can share state
//! - The `skip!` macro works correctly in async context

use std::cell::RefCell;

use rstest_bdd::skip;
use rstest_bdd_macros::{given, scenarios, then, when};

thread_local! {
    static ASYNC_COUNTER: RefCell<i32> = const { RefCell::new(0) };
}

#[given("an async counter is initialised to 0")]
fn async_counter_init() {
    ASYNC_COUNTER.with(|c| *c.borrow_mut() = 0);
}

#[when("the async counter is incremented")]
fn async_counter_increment() {
    ASYNC_COUNTER.with(|c| *c.borrow_mut() += 1);
}

#[when("the async step requests skip")]
fn async_step_skip() {
    skip!("Skipping from async context");
}

#[then(expr = "the async counter value is {n}")]
fn async_counter_value(n: i32) {
    ASYNC_COUNTER.with(|c| {
        let val = *c.borrow();
        assert_eq!(val, n, "expected counter to be {n}, got {val}");
    });
}

scenarios!(
    "tests/features/async_scenario.feature",
    tags = "@async",
    runtime = "tokio-current-thread"
);
