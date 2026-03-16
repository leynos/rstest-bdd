//! Tests for async scenario execution using manual `#[scenario]` with `#[tokio::test]`.
//!
//! This test verifies that:
//! - Manual async scenario tests with `#[tokio::test]` work correctly
//! - Async steps execute sequentially and can share state
//! - The `skip!` macro works correctly in async context
//!
//! Note: The legacy `runtime = "tokio-current-thread"` syntax for `scenarios!` is
//! deprecated and now resolves to synchronous scenarios with `TokioHarness`, which
//! does not support async step definitions. For async step functions, use manual
//! `#[scenario]` with `#[tokio::test]` instead.

use std::cell::RefCell;

use rstest_bdd::skip;
use rstest_bdd_macros::{given, scenario, then, when};

thread_local! {
    static ASYNC_COUNTER: RefCell<i32> = const { RefCell::new(0) };
}

#[given("an async counter is initialised to 0")]
fn async_counter_init() {
    ASYNC_COUNTER.with(|c| *c.borrow_mut() = 0);
}

#[when("the async counter is incremented")]
async fn async_counter_increment() {
    ASYNC_COUNTER.with(|c| *c.borrow_mut() += 1);
    tokio::task::yield_now().await;
}

#[when("the async step requests skip")]
fn async_step_skip() {
    skip!("Skipping from async context");
}

#[when("the async counter is incremented after skip")]
fn async_counter_increment_after_skip() {
    panic!("async counter increment after skip should not run when scenario is skipped");
}

#[then(expr = "the async counter value is {n}")]
fn async_counter_value(n: i32) {
    ASYNC_COUNTER.with(|c| {
        let val = *c.borrow();
        assert_eq!(val, n, "expected counter to be {n}, got {val}");
    });
}

#[scenario(
    path = "tests/features/async_scenario.feature",
    name = "Async steps execute sequentially"
)]
#[tokio::test(flavor = "current_thread")]
async fn async_steps_execute_sequentially() {}

#[scenario(
    path = "tests/features/async_scenario.feature",
    name = "Skip works in async context"
)]
#[tokio::test(flavor = "current_thread")]
async fn skip_works_in_async_context() {}
