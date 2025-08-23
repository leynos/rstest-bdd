//! Behavioural test for step registry

use rstest_bdd::{Step, StepContext, StepError, iter, step};

fn sample() {}
#[expect(
    clippy::unnecessary_wraps,
    reason = "wrapper must match StepFn signature"
)]
fn wrapper(
    ctx: &rstest_bdd::StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<(), StepError> {
    // Adapter for zero-argument step functions
    let _ = ctx;
    sample();
    Ok(())
}

step!(rstest_bdd::StepKeyword::When, "behavioural", wrapper, &[]);

fn failing_wrapper(
    ctx: &StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<(), StepError> {
    let _ = ctx;
    Err(StepError::ExecutionError {
        pattern: "fails".into(),
        function: "failing_wrapper".into(),
        message: "boom".into(),
    })
}

step!(
    rstest_bdd::StepKeyword::Given,
    "fails",
    failing_wrapper,
    &[]
);

#[test]
fn step_is_registered() {
    let found = iter::<Step>.into_iter().any(|step| {
        step.pattern.as_str() == "behavioural" && step.keyword == rstest_bdd::StepKeyword::When
    });
    assert!(found, "expected step not found");
}

#[test]
fn wrapper_error_propagates() {
    let step_fn = iter::<Step>
        .into_iter()
        .find(|s| s.pattern.as_str() == "fails")
        .map_or_else(
            || panic!("step 'fails' not found in registry"),
            |step| step.run,
        );
    let result = step_fn(&StepContext::default(), "fails", None, None);
    let err = match result {
        Ok(()) => panic!("expected error from wrapper"),
        Err(e) => e,
    };
    match err {
        StepError::ExecutionError {
            pattern,
            function,
            message,
        } => {
            assert_eq!(pattern, "fails");
            assert_eq!(function, "failing_wrapper");
            assert_eq!(message, "boom");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
