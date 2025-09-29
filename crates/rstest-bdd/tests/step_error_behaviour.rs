//! Behavioural tests for `StepError` propagation in wrappers

mod step_error_common;

use rstest_bdd::StepKeyword;

use step_error_common::{FancyValue, StepInvocation, invoke_step};

#[test]
fn successful_step_execution() {
    let res = invoke_step(&StepInvocation::new(
        StepKeyword::Given,
        "a successful step",
        "a successful step",
    ));
    if let Err(e) = res {
        panic!("unexpected error: {e:?}");
    }
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
    let boxed = invoke_step(&StepInvocation::new(
        StepKeyword::Given,
        "a fallible value step succeeds",
        "a fallible value step succeeds",
    ))
    .unwrap_or_else(|e| panic!("unexpected error: {e:?}"))
    .unwrap_or_else(|| panic!("expected step to return a value"));
    let value = boxed
        .downcast::<FancyValue>()
        .unwrap_or_else(|_| panic!("expected FancyValue payload"));
    assert_eq!(*value, FancyValue(99));
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "test ensures datatable steps can execute successfully"
)]
fn datatable_is_passed_and_executes() {
    let table: &[&[&str]] = &[&["a", "b"], &["c", "d"]];
    invoke_step(
        &StepInvocation::new(
            StepKeyword::Given,
            "a step requiring a table",
            "a step requiring a table",
        )
        .with_datatable(table),
    )
    .expect("unexpected error passing datatable");
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "test ensures docstring steps can execute successfully"
)]
fn docstring_is_passed_and_executes() {
    invoke_step(
        &StepInvocation::new(
            StepKeyword::Given,
            "a step requiring a docstring",
            "a step requiring a docstring",
        )
        .with_docstring("content"),
    )
    .expect("unexpected error passing docstring");
}
