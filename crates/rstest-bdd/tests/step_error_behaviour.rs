//! Behavioural tests for `StepError` propagation in wrappers.

mod step_error_common;

use rstest::rstest;
use rstest_bdd::{StepExecution, StepKeyword};

use step_error_common::{FancyValue, StepInvocation, invoke_step};

#[test]
fn successful_step_execution() {
    #[expect(
        clippy::expect_used,
        reason = "test ensures successful step execution propagates"
    )]
    match invoke_step(&StepInvocation::new(
        StepKeyword::Given,
        "a successful step",
        "a successful step",
    ))
    .expect("unexpected error")
    {
        StepExecution::Continue { .. } => {}
        StepExecution::Skipped { .. } => panic!("step should not have been skipped"),
    }
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "test ensures step success is propagated"
)]
fn fallible_unit_step_execution_returns_none() {
    let outcome = invoke_step(&StepInvocation::new(
        StepKeyword::Given,
        "a fallible unit step succeeds",
        "a fallible unit step succeeds",
    ))
    .expect("unexpected error");
    match outcome {
        StepExecution::Continue { value } => {
            assert!(value.is_none(), "unit step should not return a payload");
        }
        StepExecution::Skipped { .. } => panic!("unit step should not be skipped"),
    }
}

#[test]
fn fallible_value_step_execution_returns_value() {
    #[expect(
        clippy::expect_used,
        reason = "test asserts success path and payload presence"
    )]
    let payload = invoke_step(&StepInvocation::new(
        StepKeyword::Given,
        "a fallible value step succeeds",
        "a fallible value step succeeds",
    ))
    .expect("unexpected error");
    let boxed = match payload {
        StepExecution::Continue { value: Some(value) } => value,
        StepExecution::Continue { value: None } => {
            panic!("expected step to return a value")
        }
        StepExecution::Skipped { .. } => panic!("step unexpectedly skipped"),
    };
    #[expect(
        clippy::expect_used,
        reason = "test asserts success path and payload presence"
    )]
    let value = boxed
        .downcast::<FancyValue>()
        .expect("expected FancyValue payload");
    assert_eq!(*value, FancyValue(99));
}

#[test]
fn skip_request_step_returns_skipped_outcome() {
    #[expect(
        clippy::expect_used,
        reason = "test asserts skip handling returns a skipped outcome"
    )]
    let outcome = invoke_step(&StepInvocation::new(
        StepKeyword::Given,
        "a skip request step",
        "a skip request step",
    ))
    .expect("unexpected step error");
    match outcome {
        StepExecution::Continue { .. } => {
            panic!("skip request should not report continuation");
        }
        StepExecution::Skipped { message } => {
            #[expect(clippy::expect_used, reason = "test asserts skip message propagation")]
            let detail = message.expect("skip should include message");
            assert!(
                detail.contains("behavioural skip test"),
                "skip message should propagate details: {detail}",
            );
        }
    }
}

enum Payload<'a> {
    Table(&'a [&'a [&'a str]]),
    Docstring(&'a str),
}

#[rstest]
#[case::table(Payload::Table(&[&["a", "b"], &["c", "d"]]))]
#[case::docstring(Payload::Docstring("content"))]
fn datatable_or_docstring_executes(#[case] payload: Payload<'_>) {
    let invocation = match payload {
        Payload::Table(table) => StepInvocation::new(
            StepKeyword::Given,
            "a step requiring a table",
            "a step requiring a table",
        )
        .with_datatable(table),
        Payload::Docstring(text) => StepInvocation::new(
            StepKeyword::Given,
            "a step requiring a docstring",
            "a step requiring a docstring",
        )
        .with_docstring(text),
    };
    #[expect(
        clippy::expect_used,
        reason = "test ensures both table and docstring steps execute successfully"
    )]
    match invoke_step(&invocation).expect("unexpected error passing payload") {
        StepExecution::Continue { .. } => {}
        StepExecution::Skipped { .. } => panic!("step unexpectedly skipped"),
    }
}
