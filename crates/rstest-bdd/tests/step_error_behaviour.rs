//! Behavioural tests for `StepError` propagation in wrappers

use rstest::rstest;
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

#[rstest]
#[case(
    "a failing step",
    "failing_step",
    StepError::ExecutionError {
        pattern: "a failing step".into(),
        function: "failing_step".into(),
        message: "boom".into(),
    },
)]
#[case(
    "a panicking step",
    "panicking_step",
    StepError::PanicError {
        pattern: "a panicking step".into(),
        function: "panicking_step".into(),
        message: "kaboom".into(),
    },
)]
#[case(
    "a non-string panicking step",
    "non_string_panicking_step",
    StepError::PanicError {
        pattern: "a non-string panicking step".into(),
        function: "non_string_panicking_step".into(),
        message: "123".into(),
    },
)]
#[case(
    "a step requiring a table",
    "step_needing_table",
    StepError::ExecutionError {
        pattern: "a step requiring a table".into(),
        function: "step_needing_table".into(),
        message: "Step 'a step requiring a table' requires a data table".into(),
    },
)]
#[case(
    "a step requiring a docstring",
    "step_needing_docstring",
    StepError::ExecutionError {
        pattern: "a step requiring a docstring".into(),
        function: "step_needing_docstring".into(),
        message: "Step 'a step requiring a docstring' requires a doc string".into(),
    },
)]
fn step_error_scenarios(
    #[case] step_pattern: &str,
    #[case] expected_function: &str,
    #[case] expected_error: StepError,
) {
    let ctx = StepContext::default();
    let step_fn = rstest_bdd::lookup_step(StepKeyword::Given, step_pattern.into())
        .unwrap_or_else(|| panic!("step '{step_pattern}' not found in registry"));
    let err = match step_fn(&ctx, step_pattern, None, None) {
        Ok(()) => panic!("expected error for '{step_pattern}'"),
        Err(e) => e,
    };
    match (err, expected_error) {
        (
            StepError::ExecutionError {
                pattern,
                function,
                message,
            },
            StepError::ExecutionError {
                message: expected_message,
                ..
            },
        )
        | (
            StepError::PanicError {
                pattern,
                function,
                message,
            },
            StepError::PanicError {
                message: expected_message,
                ..
            },
        ) => {
            assert_eq!(pattern, step_pattern);
            assert_eq!(function, expected_function);
            assert_eq!(message, expected_message);
        }
        (other_actual, other_expected) => panic!(
            "unexpected error for '{step_pattern}': got {other_actual:?}, expected {other_expected:?}",
        ),
    }
}

#[test]
fn successful_step_execution() {
    let ctx = StepContext::default();
    let step_fn = rstest_bdd::lookup_step(StepKeyword::Given, "a successful step".into())
        .unwrap_or_else(|| panic!("step 'a successful step' not found in registry"));
    let res = step_fn(&ctx, "a successful step", None, None);
    if let Err(e) = res {
        panic!("unexpected error: {e:?}");
    }
}

#[test]
fn datatable_is_passed_and_executes() {
    let ctx = StepContext::default();
    let step_fn = rstest_bdd::lookup_step(StepKeyword::Given, "a step requiring a table".into())
        .unwrap_or_else(|| panic!("step 'a step requiring a table' not found in registry"));

    // Minimal 2×2 data table
    let table: &[&[&str]] = &[&["a", "b"], &["c", "d"]];
    if let Err(e) = step_fn(&ctx, "a step requiring a table", None, Some(table)) {
        panic!("unexpected error passing datatable: {e:?}");
    }
}

#[test]
fn docstring_is_passed_and_executes() {
    let ctx = StepContext::default();
    let step_fn =
        rstest_bdd::lookup_step(StepKeyword::Given, "a step requiring a docstring".into())
            .unwrap_or_else(|| panic!("step 'a step requiring a docstring' not found in registry"));

    if let Err(e) = step_fn(&ctx, "a step requiring a docstring", Some("content"), None) {
        panic!("unexpected error passing docstring: {e:?}");
    }
}
