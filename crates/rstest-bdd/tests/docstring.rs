//! Behavioural test for doc string support

use std::cell::RefCell;

use rstest_bdd_macros::{given, scenario, then};

thread_local! {
    #[expect(
        clippy::missing_const_for_thread_local,
        reason = "const RefCell::new(None) would raise MSRV"
    )]
    // FIXME: https://github.com/leynos/rstest-bdd/issues/54
    static CAPTURED: RefCell<Option<String>> = RefCell::new(None);
}

#[given("the following message:")]
fn capture_message(docstring: String) {
    CAPTURED.with(|m| {
        m.replace(Some(docstring));
    });
}

#[then("the captured message equals:")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "doc string is owned to mirror user API"
)]
#[expect(clippy::expect_used, reason = "test ensures a message was captured")]
fn assert_message(docstring: String) {
    CAPTURED.with(|m| {
        let captured = m
            .borrow_mut()
            .take()
            .expect("message should be captured before assertion");
        assert_eq!(captured, docstring);
    });
}

#[given("message then value {int}:")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "doc string is owned to mirror user API"
)]
fn doc_then_value(docstring: String, value: i32) {
    assert_eq!(docstring.trim(), "alpha");
    assert_eq!(value, 5);
}

#[given("value then message {int}:")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "doc string is owned to mirror user API"
)]
fn value_then_doc(value: i32, docstring: String) {
    assert_eq!(value, 5);
    assert_eq!(docstring.trim(), "alpha");
}

#[scenario(path = "tests/features/docstring.feature")]
fn docstring_scenario() {}

#[scenario(path = "tests/features/background_docstring.feature")]
fn background_docstring_scenario() {}

#[scenario(path = "tests/features/backticks_docstring.feature")]
fn backticks_docstring_scenario() {}

#[scenario(path = "tests/features/missing_docstring.feature")]
#[should_panic(expected = "requires a doc string")]
fn missing_docstring_scenario() {}

#[scenario(path = "tests/features/docstring_arg_order.feature")]
fn docstring_arg_order_scenario() {}
