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

fn panicking_wrapper(
    ctx: &StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<(), StepError> {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let _ = ctx;
    catch_unwind(AssertUnwindSafe(|| panic!("snap"))).map_err(|e| {
        #[expect(
            clippy::option_if_let_else,
            reason = "sequential downcasts aid readability"
        )]
        let message = if let Some(s) = e.downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = e.downcast_ref::<String>() {
            s.clone()
        } else if let Some(i) = e.downcast_ref::<i32>() {
            i.to_string()
        } else if let Some(i) = e.downcast_ref::<u32>() {
            i.to_string()
        } else if let Some(i) = e.downcast_ref::<i64>() {
            i.to_string()
        } else if let Some(i) = e.downcast_ref::<u64>() {
            i.to_string()
        } else if let Some(i) = e.downcast_ref::<isize>() {
            i.to_string()
        } else if let Some(i) = e.downcast_ref::<usize>() {
            i.to_string()
        } else if let Some(f) = e.downcast_ref::<f64>() {
            f.to_string()
        } else if let Some(f) = e.downcast_ref::<f32>() {
            f.to_string()
        } else {
            format!("{e:?}")
        };
        StepError::PanicError {
            pattern: "panics".into(),
            function: "panicking_wrapper".into(),
            message,
        }
    })?;
    Ok(())
}

step!(
    rstest_bdd::StepKeyword::When,
    "panics",
    panicking_wrapper,
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

#[test]
fn wrapper_panic_is_captured() {
    let step_fn = iter::<Step>
        .into_iter()
        .find(|s| s.pattern.as_str() == "panics")
        .map_or_else(
            || panic!("step 'panics' not found in registry"),
            |step| step.run,
        );
    let err = match step_fn(&StepContext::default(), "panics", None, None) {
        Ok(()) => panic!("expected error from wrapper"),
        Err(e) => e,
    };
    match err {
        StepError::PanicError {
            pattern,
            function,
            message,
        } => {
            assert_eq!(pattern, "panics");
            assert_eq!(function, "panicking_wrapper");
            assert_eq!(message, "snap");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
