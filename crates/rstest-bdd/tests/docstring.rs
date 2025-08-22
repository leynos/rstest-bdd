//! Behavioural test for doc string support
#![expect(
    clippy::unnecessary_wraps,
    reason = "step functions must return StepError"
)]

use std::cell::RefCell;

use rstest_bdd::{Step, StepContext, StepError, iter};
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
#[expect(
    clippy::unnecessary_wraps,
    reason = "step functions must return StepError"
)]
fn capture_message(docstring: String) -> Result<(), StepError> {
    CAPTURED.with(|m| {
        m.replace(Some(docstring));
    });
    Ok(())
}

#[then("the captured message equals:")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "doc string is owned to mirror user API"
)]
#[expect(clippy::expect_used, reason = "test ensures a message was captured")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step functions must return StepError"
)]
fn assert_message(docstring: String) -> Result<(), StepError> {
    CAPTURED.with(|m| {
        let captured = m
            .borrow_mut()
            .take()
            .expect("message should be captured before assertion");
        assert_eq!(captured, docstring);
    });
    Ok(())
}

#[given("message then value {int}:")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "doc string is owned to mirror user API"
)]
fn doc_then_value(docstring: String, value: i32) -> Result<(), StepError> {
    assert_eq!(docstring.trim(), "alpha");
    assert_eq!(value, 5);
    Ok(())
}

#[given("value then message {int}:")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "doc string is owned to mirror user API"
)]
fn value_then_doc(value: i32, docstring: String) -> Result<(), StepError> {
    assert_eq!(value, 5);
    assert_eq!(docstring.trim(), "alpha");
    Ok(())
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

#[test]
fn missing_docstring_returns_execution_error() {
    let step_fn = iter::<Step>
        .into_iter()
        .find(|s| s.pattern.as_str() == "the following message:")
        .map_or_else(
            || panic!("step 'the following message:' not found in registry"),
            |step| step.run,
        );
    let result = step_fn(
        &StepContext::default(),
        "the following message:",
        None,
        None,
    );
    let err = match result {
        Ok(()) => panic!("expected error when doc string is missing"),
        Err(e) => e,
    };
    match err {
        StepError::ExecutionError { step, message } => {
            assert_eq!(step, "capture_message");
            assert!(message.contains("requires a doc string"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
