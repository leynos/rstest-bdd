//! Keyword-focused step-error scenarios exercising the `ExecutionError`, `PanicError`, and
//! `MissingFixture` variants.

mod step_error_common;

use rstest::rstest;
use rstest_bdd::{StepError, StepKeyword};
use step_error_common::{StepInvocation, invoke_step};

/// A registered step to invoke, together with the function it should name.
#[derive(Clone, Copy)]
struct InvokedStep<'a> {
    keyword: StepKeyword,
    pattern: &'a str,
    text: &'a str,
    function: &'a str,
}

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
            assert_eq!(pattern, step_pattern, "unexpected pattern");
            assert_eq!(function, expected_function, "unexpected function");
            assert_eq!(message, expected_message, "unexpected message");
        }
        (
            StepError::MissingFixture { name, ty, step },
            StepError::MissingFixture {
                name: expected_name,
                ty: expected_ty,
                step: expected_step,
            },
        ) => {
            assert_eq!(name, expected_name, "unexpected name");
            assert_eq!(ty, expected_ty, "unexpected ty");
            assert_eq!(step, expected_step, "unexpected step");
        }
        (other_actual, other_expected) => panic!(
            "unexpected error for {step_pattern}: got {other_actual:?}, expected \
             {other_expected:?}"
        ),
    }
}

#[rstest]
#[case(
    InvokedStep {
        keyword: StepKeyword::Given,
        pattern: "a failing step",
        text: "a failing step",
        function: "failing_step",
    },
    StepError::ExecutionError {
        pattern: "a failing step".into(),
        function: "failing_step".into(),
        message: "boom".into(),
    },
)]
#[case(
    InvokedStep {
        keyword: StepKeyword::Given,
        pattern: "an alias error step",
        text: "an alias error step",
        function: "alias_error_step",
    },
    StepError::ExecutionError {
        pattern: "an alias error step".into(),
        function: "alias_error_step".into(),
        message: "alias boom".into(),
    },
)]
#[case(
    InvokedStep {
        keyword: StepKeyword::Given,
        pattern: "a fallible unit step fails",
        text: "a fallible unit step fails",
        function: "fallible_unit_step_fails",
    },
    StepError::ExecutionError {
        pattern: "a fallible unit step fails".into(),
        function: "fallible_unit_step_fails".into(),
        message: "unit failure".into(),
    },
)]
#[case(
    InvokedStep {
        keyword: StepKeyword::Given,
        pattern: "a fallible value step fails",
        text: "a fallible value step fails",
        function: "fallible_value_step_fails",
    },
    StepError::ExecutionError {
        pattern: "a fallible value step fails".into(),
        function: "fallible_value_step_fails".into(),
        message: "value failure".into(),
    },
)]
#[case(
    InvokedStep {
        keyword: StepKeyword::Given,
        pattern: "a panicking step",
        text: "a panicking step",
        function: "panicking_step",
    },
    StepError::PanicError {
        pattern: "a panicking step".into(),
        function: "panicking_step".into(),
        message: "kaboom".into(),
    },
)]
#[case(
    InvokedStep {
        keyword: StepKeyword::Given,
        pattern: "a non-string panicking step",
        text: "a non-string panicking step",
        function: "non_string_panicking_step",
    },
    StepError::PanicError {
        pattern: "a non-string panicking step".into(),
        function: "non_string_panicking_step".into(),
        message: "123".into(),
    },
)]
#[case(
    InvokedStep {
        keyword: StepKeyword::Given,
        pattern: "a step requiring a table",
        text: "a step requiring a table",
        function: "step_needing_table",
    },
    StepError::ExecutionError {
        pattern: "a step requiring a table".into(),
        function: "step_needing_table".into(),
        message: "Step 'a step requiring a table' requires a data table".into(),
    },
)]
#[case(
    InvokedStep {
        keyword: StepKeyword::Given,
        pattern: "a step requiring a docstring",
        text: "a step requiring a docstring",
        function: "step_needing_docstring",
    },
    StepError::ExecutionError {
        pattern: "a step requiring a docstring".into(),
        function: "step_needing_docstring".into(),
        message: "Step 'a step requiring a docstring' requires a doc string".into(),
    },
)]
#[case(
    InvokedStep {
        keyword: StepKeyword::Given,
        pattern: "number {value}",
        text: "number not_a_number",
        function: "parse_number",
    },
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
    InvokedStep {
        keyword: StepKeyword::Given,
        pattern: "no placeholders",
        text: "no placeholders",
        function: "missing_capture",
    },
    StepError::MissingFixture {
        name: "value".into(),
        ty: "u32".into(),
        step: "missing_capture".into(),
    },
)]
#[case(
    InvokedStep {
        keyword: StepKeyword::When,
        pattern: "a failing when step",
        text: "a failing when step",
        function: "failing_when_step",
    },
    StepError::ExecutionError {
        pattern: "a failing when step".into(),
        function: "failing_when_step".into(),
        message: "when boom".into(),
    },
)]
#[case(
    InvokedStep {
        keyword: StepKeyword::Then,
        pattern: "a failing then step",
        text: "a failing then step",
        function: "failing_then_step",
    },
    StepError::ExecutionError {
        pattern: "a failing then step".into(),
        function: "failing_then_step".into(),
        message: "then boom".into(),
    },
)]
fn step_error_scenarios(#[case] invoked: InvokedStep<'_>, #[case] expected_error: StepError) {
    let InvokedStep {
        keyword,
        pattern: step_pattern,
        text: step_text,
        function: expected_function,
    } = invoked;
    let Err(err) = invoke_step(&StepInvocation::new(keyword, step_pattern, step_text)) else {
        panic!("expected error for '{step_text}'");
    };
    assert_step_error(&err, expected_function, step_pattern, &expected_error);
}

#[test]
fn invocation_builder_supports_optionals() {
    let table: &[&[&str]] = &[&["value"]];
    let invocation =
        StepInvocation::new(StepKeyword::Given, "optional pattern", "optional pattern")
            .with_docstring("doc")
            .with_datatable(table);
    assert_eq!(invocation.docstring, Some("doc"));
    assert!(invocation.datatable.is_some());
}
