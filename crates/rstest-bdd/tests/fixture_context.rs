//! Behavioural test for fixture context injection

use rstest_bdd::{Step, StepContext, StepError, iter, step};

fn needs_value(
    ctx: &StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<(), StepError> {
    let val = ctx
        .get::<u32>("number")
        .ok_or_else(|| StepError::MissingFixture {
            name: "number".to_string(),
            ty: "u32".to_string(),
            step: "needs_value".to_string(),
        })?;
    assert_eq!(*val, 42);
    Ok(())
}

step!(
    rstest_bdd::StepKeyword::Given,
    "a value",
    needs_value,
    &["number"]
);

#[test]
fn context_passes_fixture() {
    let mut ctx = StepContext::default();
    let number = 42u32;
    ctx.insert("number", &number);
    let step_fn = iter::<Step>
        .into_iter()
        .find(|s| s.pattern.as_str() == "a value")
        .map_or_else(
            || panic!("step 'a value' not found in registry"),
            |step| step.run,
        );
    let result = step_fn(&ctx, "a value", None, None);
    assert!(result.is_ok(), "step execution failed: {result:?}");
}

#[test]
fn context_missing_fixture_returns_error() {
    let ctx = StepContext::default();
    let step_fn = iter::<Step>
        .into_iter()
        .find(|s| s.pattern.as_str() == "a value")
        .map_or_else(
            || panic!("step 'a value' not found in registry"),
            |step| step.run,
        );
    let result = step_fn(&ctx, "a value", None, None);
    let err = match result {
        Ok(()) => panic!("expected error when fixture is missing"),
        Err(e) => e,
    };
    match err {
        StepError::MissingFixture { name, ty, step } => {
            assert_eq!(name, "number");
            assert_eq!(ty, "u32");
            assert_eq!(step, "needs_value");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

fn panicky_core() {
    panic!("boom");
}

fn panicky_step(
    _ctx: &StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<(), StepError> {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    catch_unwind(AssertUnwindSafe(panicky_core)).map_err(|e| {
        let message = e
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| e.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| format!("{e:?}"));
        StepError::PanicError {
            pattern: "it panics".to_string(),
            function: "panicky_core".to_string(),
            message,
        }
    })
}

step!(
    rstest_bdd::StepKeyword::When,
    "it panics",
    panicky_step,
    &[]
);

#[test]
fn panic_is_reported() {
    let step_fn = iter::<Step>
        .into_iter()
        .find(|s| s.pattern.as_str() == "it panics")
        .map_or_else(
            || panic!("step 'it panics' not found in registry"),
            |step| step.run,
        );
    let result = step_fn(&StepContext::default(), "it panics", None, None);
    let err = match result {
        Ok(()) => panic!("expected panic error"),
        Err(e) => e,
    };
    match err {
        StepError::PanicError {
            pattern,
            function,
            message,
        } => {
            assert_eq!(pattern, "it panics");
            assert_eq!(function, "panicky_core");
            assert_eq!(message, "boom");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
