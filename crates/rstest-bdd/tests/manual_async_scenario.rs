//! Tests for manual async scenario using `#[scenario]` with `#[tokio::test]`.
//!
//! This test verifies that:
//! - The `#[scenario]` macro detects async function signatures
//! - Users can combine `#[scenario]` with `#[tokio::test]` for async tests
//! - The async step executor is used when the function is async

use std::cell::RefCell;

use rstest_bdd_macros::{given, scenario, then, when};

thread_local! {
    static MANUAL_ASYNC_STATE: RefCell<String> = const { RefCell::new(String::new()) };
}

#[given("the manual async step runs")]
fn manual_async_given() {
    MANUAL_ASYNC_STATE.with(|s| *s.borrow_mut() = "given".to_string());
}

#[when("the manual async step continues")]
fn manual_async_when() {
    MANUAL_ASYNC_STATE.with(|s| s.borrow_mut().push_str(" -> when"));
}

#[then("the manual async step completes")]
fn manual_async_then() {
    MANUAL_ASYNC_STATE.with(|s| {
        let state = s.borrow();
        assert_eq!(*state, "given -> when");
    });
}

#[scenario(path = "tests/features/manual_async_scenario.feature")]
#[tokio::test(flavor = "current_thread")]
async fn manual_async_scenario_test() {
    // The steps execute before this body
    // This body runs after all steps complete successfully
}

#[given("a failing manual async step")]
fn manual_async_failing_step() {
    panic!("manual async failing step panicked");
}

#[scenario(
    path = "tests/features/manual_async_failing_scenario.feature",
    name = "Manual async scenario with failing step"
)]
#[tokio::test(flavor = "current_thread")]
#[should_panic(expected = "manual async failing step panicked")]
async fn manual_async_failing_scenario_test() {
    // The steps execute before this body.
    // The failing step should cause this test to panic,
    // and the test harness will verify the panic message.
}
