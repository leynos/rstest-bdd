//! Behavioural test for doc string support

use std::cell::RefCell;

use rstest_bdd_macros::{given, scenario, then};

thread_local! {
    static CAPTURED: RefCell<Option<String>> = const { RefCell::new(None) };
}

#[given("the following message:")]
fn capture_message(docstring: String) {
    CAPTURED.with(|m| {
        m.borrow_mut().replace(docstring);
    });
}

#[then("the captured message equals:")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "doc string is owned to mirror user API"
)]
fn assert_message(docstring: String) {
    CAPTURED.with(|m| {
        let Some(captured) = m.borrow_mut().take() else {
            panic!("message should be captured before assertion");
        };
        assert_eq!(captured, docstring);
    });
}

#[scenario(path = "tests/features/docstring.feature")]
fn docstring_scenario() {}

#[scenario(path = "tests/features/missing_docstring.feature")]
#[should_panic(expected = "requires a doc string")]
fn missing_docstring_scenario() {}
