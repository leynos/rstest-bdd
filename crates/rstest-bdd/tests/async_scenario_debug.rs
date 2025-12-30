//! Debug test for async scenario execution.
//!
//! Using manual #[scenario] to get better error messages.

use std::cell::RefCell;

use rstest_bdd_macros::{given, scenario, then, when};

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
#[tokio::test]
async fn async_steps_execute_sequentially() {}
