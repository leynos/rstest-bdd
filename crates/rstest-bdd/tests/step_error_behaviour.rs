//! Behavioural tests for `StepError` propagation in wrappers.

mod step_error_common;

use rstest::rstest;
use rstest_bdd::StepKeyword;

use step_error_common::{FancyValue, StepInvocation, invoke_step};

#[test]
fn successful_step_execution() {
    #[expect(
        clippy::expect_used,
        reason = "test ensures successful step execution propagates"
    )]
    invoke_step(&StepInvocation::new(
        StepKeyword::Given,
        "a successful step",
        "a successful step",
    ))
    .expect("unexpected error");
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "test ensures step success is propagated"
)]
fn fallible_unit_step_execution_returns_none() {
    let res = invoke_step(&StepInvocation::new(
        StepKeyword::Given,
        "a fallible unit step succeeds",
        "a fallible unit step succeeds",
    ))
    .expect("unexpected error");
    assert!(res.is_none(), "unit step should not return a payload");
}

#[test]
fn fallible_value_step_execution_returns_value() {
    #[expect(
        clippy::expect_used,
        reason = "test asserts success path and payload presence"
    )]
    let boxed = invoke_step(&StepInvocation::new(
        StepKeyword::Given,
        "a fallible value step succeeds",
        "a fallible value step succeeds",
    ))
    .expect("unexpected error")
    .expect("expected step to return a value");
    #[expect(
        clippy::expect_used,
        reason = "test asserts success path and payload presence"
    )]
    let value = boxed
        .downcast::<FancyValue>()
        .expect("expected FancyValue payload");
    assert_eq!(*value, FancyValue(99));
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
    invoke_step(&invocation).expect("unexpected error passing payload");
}
