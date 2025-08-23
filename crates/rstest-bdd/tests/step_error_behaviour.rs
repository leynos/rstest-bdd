//! Behavioural tests for `StepError` propagation in wrappers

use rstest_bdd::{StepContext, StepError, StepKeyword};
use rstest_bdd_macros::given;

#[given("a failing step")]
fn failing_step() -> Result<(), String> {
    Err("boom".into())
}

#[given("a panicking step")]
fn panicking_step() -> Result<(), String> {
    panic!("kaboom")
}

#[test]
fn execution_error_is_reported() {
    let ctx = StepContext::default();
    let step_fn = rstest_bdd::lookup_step(StepKeyword::Given, "a failing step".into())
        .unwrap_or_else(|| panic!("step 'a failing step' not found in registry"));
    let err = match step_fn(&ctx, "a failing step", None, None) {
        Ok(()) => panic!("expected error"),
        Err(e) => e,
    };
    match err {
        StepError::ExecutionError {
            pattern,
            function,
            message,
        } => {
            assert_eq!(pattern, "a failing step");
            assert_eq!(function, "failing_step");
            assert_eq!(message, "boom");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn panic_is_captured() {
    let ctx = StepContext::default();
    let step_fn = rstest_bdd::lookup_step(StepKeyword::Given, "a panicking step".into())
        .unwrap_or_else(|| panic!("step 'a panicking step' not found in registry"));
    let err = match step_fn(&ctx, "a panicking step", None, None) {
        Ok(()) => panic!("expected error"),
        Err(e) => e,
    };
    match err {
        StepError::PanicError {
            pattern,
            function,
            message,
        } => {
            assert_eq!(pattern, "a panicking step");
            assert_eq!(function, "panicking_step");
            assert_eq!(message, "kaboom");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
