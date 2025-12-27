//! Integration tests for the :string type hint.
//!
//! These tests verify that the complete code-generation pipeline correctly
//! handles the :string type hint, including quote stripping at runtime.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then};
use std::cell::RefCell;

#[fixture]
fn message() -> RefCell<String> {
    RefCell::new(String::new())
}

/// Step that captures a quoted string and strips the quotes.
#[given("the message is {text:string}")]
fn set_message(message: &RefCell<String>, text: &str) {
    // The :string hint should have stripped the surrounding quotes
    *message.borrow_mut() = text.to_string();
}

/// Verify the message was captured without surrounding quotes.
#[then("the parsed message is {expected:string}")]
fn check_message(message: &RefCell<String>, expected: &str) {
    assert_eq!(
        *message.borrow(),
        expected,
        "message should match without quotes"
    );
}

#[scenario(
    path = "tests/features/string_hint.feature",
    name = "Parse quoted string with double quotes"
)]
fn double_quoted_string(message: RefCell<String>) {
    let _ = message;
}

#[scenario(
    path = "tests/features/string_hint.feature",
    name = "Parse quoted string with single quotes"
)]
fn single_quoted_string(message: RefCell<String>) {
    let _ = message;
}

#[scenario(
    path = "tests/features/string_hint.feature",
    name = "Parse empty quoted string"
)]
fn empty_quoted_string(message: RefCell<String>) {
    let _ = message;
}
