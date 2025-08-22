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
            name: "number".into(),
            ty: "u32".into(),
            step: "needs_value".into(),
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
    catch_unwind(AssertUnwindSafe(panicky_core)).map_err(|e| StepError::PanicError {
        pattern: "it panics".into(),
        function: "panicky_core".into(),
        message: rstest_bdd::panic_message(e.as_ref()),
    })
}

step!(
    rstest_bdd::StepKeyword::When,
    "it panics",
    panicky_step,
    &[]
);

fn panicky_core_non_string() {
    std::panic::panic_any(42u8);
}

fn panicky_step_non_string(
    _ctx: &StepContext<'_>,
    text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<(), StepError> {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    catch_unwind(AssertUnwindSafe(panicky_core_non_string)).map_err(|e| StepError::PanicError {
        pattern: text.into(),
        function: "panicky_core_non_string".into(),
        message: rstest_bdd::panic_message(e.as_ref()),
    })
}

step!(
    rstest_bdd::StepKeyword::When,
    "it panics (non-string)",
    panicky_step_non_string,
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

#[test]
fn panic_non_string_payload_is_reported() {
    let step_fn = iter::<Step>
        .into_iter()
        .find(|s| s.pattern.as_str() == "it panics (non-string)")
        .map_or_else(
            || panic!("step 'it panics (non-string)' not found in registry"),
            |step| step.run,
        );
    let result = step_fn(
        &StepContext::default(),
        "it panics (non-string)",
        None,
        None,
    );
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
            assert_eq!(pattern, "it panics (non-string)");
            assert_eq!(function, "panicky_core_non_string");
            assert!(
                message.contains("42"),
                "message should include payload: {message}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
