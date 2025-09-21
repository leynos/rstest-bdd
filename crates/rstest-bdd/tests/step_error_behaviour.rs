//! Behavioural tests for `StepError` propagation in wrappers

use rstest::rstest;
use rstest_bdd::{StepContext, StepError, StepKeyword};
use rstest_bdd_macros::{given, then, when};
use std::fmt;

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

#[when("a failing when step")]
fn failing_when_step() -> Result<(), String> {
    Err("when boom".into())
}

#[then("a failing then step")]
fn failing_then_step() -> Result<(), String> {
    Err("then boom".into())
}

type StepResult<T> = Result<T, &'static str>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FancyValue(u16);

#[derive(Debug, PartialEq, Eq)]
struct FancyError(&'static str);

impl fmt::Display for FancyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

#[given("an alias error step")]
fn alias_error_step() -> StepResult<()> {
    Err("alias boom")
}

#[given("a fallible unit step succeeds")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step intentionally returns Result to exercise IntoStepResult"
)]
fn fallible_unit_step_succeeds() -> Result<(), FancyError> {
    Ok(())
}

#[given("a fallible unit step fails")]
fn fallible_unit_step_fails() -> Result<(), FancyError> {
    Err(FancyError("unit failure"))
}

#[given("a fallible value step succeeds")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step intentionally returns Result to exercise IntoStepResult"
)]
fn fallible_value_step_succeeds() -> Result<FancyValue, FancyError> {
    Ok(FancyValue(99))
}

#[given("a fallible value step fails")]
fn fallible_value_step_fails() -> Result<FancyValue, FancyError> {
    Err(FancyError("value failure"))
}

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

#[given("number {value}")]
fn parse_number(value: u32) {
    let _ = value;
}

#[given("no placeholders")]
fn missing_capture(value: u32) {
    let _ = value;
}

/// Assert that two `StepError` values represent the same failure.
fn assert_step_error(
    actual: &StepError,
    expected_function: &str,
    step_pattern: &str,
    expected: &StepError,
) {
    match (actual, expected) {
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
        (
            StepError::MissingFixture { name, ty, step },
            StepError::MissingFixture {
                name: expected_name,
                ty: expected_ty,
                step: expected_step,
            },
        ) => {
            assert_eq!(name, expected_name);
            assert_eq!(ty, expected_ty);
            assert_eq!(step, expected_step);
        }
        (other_actual, other_expected) => panic!(
            "unexpected error for {step_pattern}: got {other_actual:?}, expected {other_expected:?}"
        ),
    }
}

fn invoke_step(
    keyword: StepKeyword,
    step_pattern: &str,
    step_text: &str,
    docstring: Option<&str>,
    datatable: Option<&[&[&str]]>,
) -> Result<Option<Box<dyn std::any::Any>>, StepError> {
    let ctx = StepContext::default();
    let step_fn = rstest_bdd::lookup_step(keyword, step_pattern.into())
        .unwrap_or_else(|| panic!("step '{step_pattern}' not found in registry"));
    step_fn(&ctx, step_text, docstring, datatable)
}

#[rstest]
#[case(
    StepKeyword::Given,
    "a failing step",
    "a failing step",
    "failing_step",
    StepError::ExecutionError {
        pattern: "a failing step".into(),
        function: "failing_step".into(),
        message: "boom".into(),
    },
)]
#[case(
    StepKeyword::Given,
    "an alias error step",
    "an alias error step",
    "alias_error_step",
    StepError::ExecutionError {
        pattern: "an alias error step".into(),
        function: "alias_error_step".into(),
        message: "alias boom".into(),
    },
)]
#[case(
    StepKeyword::Given,
    "a fallible unit step fails",
    "a fallible unit step fails",
    "fallible_unit_step_fails",
    StepError::ExecutionError {
        pattern: "a fallible unit step fails".into(),
        function: "fallible_unit_step_fails".into(),
        message: "unit failure".into(),
    },
)]
#[case(
    StepKeyword::Given,
    "a fallible value step fails",
    "a fallible value step fails",
    "fallible_value_step_fails",
    StepError::ExecutionError {
        pattern: "a fallible value step fails".into(),
        function: "fallible_value_step_fails".into(),
        message: "value failure".into(),
    },
)]
#[case(
    StepKeyword::Given,
    "a panicking step",
    "a panicking step",
    "panicking_step",
    StepError::PanicError {
        pattern: "a panicking step".into(),
        function: "panicking_step".into(),
        message: "kaboom".into(),
    },
)]
#[case(
    StepKeyword::Given,
    "a non-string panicking step",
    "a non-string panicking step",
    "non_string_panicking_step",
    StepError::PanicError {
        pattern: "a non-string panicking step".into(),
        function: "non_string_panicking_step".into(),
        message: "123".into(),
    },
)]
#[case(
    StepKeyword::Given,
    "a step requiring a table",
    "a step requiring a table",
    "step_needing_table",
    StepError::ExecutionError {
        pattern: "a step requiring a table".into(),
        function: "step_needing_table".into(),
        message: "Step 'a step requiring a table' requires a data table".into(),
    },
)]
#[case(
    StepKeyword::Given,
    "a step requiring a docstring",
    "a step requiring a docstring",
    "step_needing_docstring",
    StepError::ExecutionError {
        pattern: "a step requiring a docstring".into(),
        function: "step_needing_docstring".into(),
        message: "Step 'a step requiring a docstring' requires a doc string".into(),
    },
)]
#[case(
    StepKeyword::Given,
    "number {value}",
    "number not_a_number",
    "parse_number",
    StepError::ExecutionError {
        pattern: "number {value}".into(),
        function: "parse_number".into(),
        message: concat!(
            "failed to parse argument 'value' of type 'u32' from pattern 'number {value}' ",
            "with captured value: '\"not_a_number\"'",
        )
        .into(),
    },
)]
#[case(
    StepKeyword::Given,
    "no placeholders",
    "no placeholders",
    "missing_capture",
    StepError::MissingFixture {
        name: "value".into(),
        ty: "u32".into(),
        step: "missing_capture".into(),
    },
)]
#[case(
    StepKeyword::When,
    "a failing when step",
    "a failing when step",
    "failing_when_step",
    StepError::ExecutionError {
        pattern: "a failing when step".into(),
        function: "failing_when_step".into(),
        message: "when boom".into(),
    },
)]
#[case(
    StepKeyword::Then,
    "a failing then step",
    "a failing then step",
    "failing_then_step",
    StepError::ExecutionError {
        pattern: "a failing then step".into(),
        function: "failing_then_step".into(),
        message: "then boom".into(),
    },
)]

fn step_error_scenarios(
    #[case] keyword: StepKeyword,
    #[case] step_pattern: &str,
    #[case] step_text: &str,
    #[case] expected_function: &str,
    #[case] expected_error: StepError,
) {
    let Err(err) = invoke_step(keyword, step_pattern, step_text, None, None) else {
        panic!("expected error for '{step_text}'");
    };
    assert_step_error(&err, expected_function, step_pattern, &expected_error);
}

#[test]
fn successful_step_execution() {
    let res = invoke_step(
        StepKeyword::Given,
        "a successful step",
        "a successful step",
        None,
        None,
    );
    if let Err(e) = res {
        panic!("unexpected error: {e:?}");
    }
}

#[test]
fn fallible_unit_step_execution_returns_none() {
    let res = invoke_step(
        StepKeyword::Given,
        "a fallible unit step succeeds",
        "a fallible unit step succeeds",
        None,
        None,
    )
    .unwrap_or_else(|e| panic!("unexpected error: {e:?}"));
    assert!(res.is_none(), "unit step should not return a payload");
}

#[test]
fn fallible_value_step_execution_returns_value() {
    let boxed = invoke_step(
        StepKeyword::Given,
        "a fallible value step succeeds",
        "a fallible value step succeeds",
        None,
        None,
    )
    .unwrap_or_else(|e| panic!("unexpected error: {e:?}"))
    .unwrap_or_else(|| panic!("expected step to return a value"));
    let value = boxed
        .downcast::<FancyValue>()
        .unwrap_or_else(|_| panic!("expected FancyValue payload"));
    assert_eq!(*value, FancyValue(99));
}

#[test]
fn datatable_is_passed_and_executes() {
    let table: &[&[&str]] = &[&["a", "b"], &["c", "d"]];
    if let Err(e) = invoke_step(
        StepKeyword::Given,
        "a step requiring a table",
        "a step requiring a table",
        None,
        Some(table),
    ) {
        panic!("unexpected error passing datatable: {e:?}");
    }
}

#[test]
fn docstring_is_passed_and_executes() {
    if let Err(e) = invoke_step(
        StepKeyword::Given,
        "a step requiring a docstring",
        "a step requiring a docstring",
        Some("content"),
        None,
    ) {
        panic!("unexpected error passing docstring: {e:?}");
    }
}
