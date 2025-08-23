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

#[given("a non-string panicking step")]
fn non_string_panicking_step() -> Result<(), String> {
    std::panic::panic_any(123_i32)
}

#[given("a successful step")]
fn successful_step() {}

#[given("a step requiring a table")]
#[expect(clippy::needless_pass_by_value, reason = "step consumes the table")]
fn step_needing_table(datatable: Vec<Vec<String>>) {
    let _ = datatable;
}

#[given("a step requiring a docstring")]
#[expect(clippy::needless_pass_by_value, reason = "step consumes docstring")]
fn step_needing_docstring(docstring: String) {
    let _ = docstring;
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

#[test]
fn non_string_panic_is_captured() {
    let ctx = StepContext::default();
    let step_fn = rstest_bdd::lookup_step(StepKeyword::Given, "a non-string panicking step".into())
        .unwrap_or_else(|| panic!("step 'a non-string panicking step' not found in registry"));
    let err = match step_fn(&ctx, "a non-string panicking step", None, None) {
        Ok(()) => panic!("expected error"),
        Err(e) => e,
    };
    match err {
        StepError::PanicError {
            pattern,
            function,
            message,
        } => {
            assert_eq!(pattern, "a non-string panicking step");
            assert_eq!(function, "non_string_panicking_step");
            assert_eq!(message, "123");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn ok_is_returned() {
    let ctx = StepContext::default();
    let step_fn = rstest_bdd::lookup_step(StepKeyword::Given, "a successful step".into())
        .unwrap_or_else(|| panic!("step 'a successful step' not found in registry"));
    let res = step_fn(&ctx, "a successful step", None, None);
    if let Err(e) = res {
        panic!("unexpected error: {e:?}");
    }
}

#[test]
fn missing_datatable_is_reported() {
    let ctx = StepContext::default();
    let step_fn = rstest_bdd::lookup_step(StepKeyword::Given, "a step requiring a table".into())
        .unwrap_or_else(|| panic!("step 'a step requiring a table' not found in registry"));
    let err = match step_fn(&ctx, "a step requiring a table", None, None) {
        Ok(()) => panic!("expected error"),
        Err(e) => e,
    };
    match err {
        StepError::ExecutionError {
            pattern,
            function,
            message,
        } => {
            assert_eq!(pattern, "a step requiring a table");
            assert_eq!(function, "step_needing_table");
            assert_eq!(
                message,
                "Step 'a step requiring a table' requires a data table",
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn missing_docstring_is_reported() {
    let ctx = StepContext::default();
    let step_fn =
        rstest_bdd::lookup_step(StepKeyword::Given, "a step requiring a docstring".into())
            .unwrap_or_else(|| panic!("step 'a step requiring a docstring' not found in registry"));
    let err = match step_fn(&ctx, "a step requiring a docstring", None, None) {
        Ok(()) => panic!("expected error"),
        Err(e) => e,
    };
    match err {
        StepError::ExecutionError {
            pattern,
            function,
            message,
        } => {
            assert_eq!(pattern, "a step requiring a docstring");
            assert_eq!(function, "step_needing_docstring");
            assert_eq!(
                message,
                "Step 'a step requiring a docstring' requires a doc string",
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
