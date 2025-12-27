//! Step wrapper definitions for registry tests.
//!
//! These wrappers are used by the step registry tests to exercise various
//! error conditions (failures, panics, missing fixtures) and the auto-generated
//! async handler feature.

use rstest_bdd::{
    StepContext, StepError, StepExecution, StepFuture, StepKeyword, panic_message, step,
};

use super::common::sync_to_async;

fn sample() {}

#[expect(
    clippy::unnecessary_wraps,
    reason = "wrapper must match StepFn signature"
)]
fn wrapper(
    ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    // Adapter for zero-argument step functions
    let _ = ctx;
    sample();
    Ok(StepExecution::from_value(None))
}

fn wrapper_async<'a>(
    ctx: &'a mut StepContext<'a>,
    text: &str,
    docstring: Option<&str>,
    table: Option<&[&[&str]]>,
) -> StepFuture<'a> {
    sync_to_async(wrapper)(ctx, text, docstring, table)
}

step!(
    StepKeyword::When,
    "behavioural",
    wrapper,
    wrapper_async,
    &[]
);

fn failing_wrapper(
    ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    let _ = ctx;
    Err(StepError::ExecutionError {
        pattern: "fails".into(),
        function: "failing_wrapper".into(),
        message: "boom".into(),
    })
}

fn failing_wrapper_async<'a>(
    ctx: &'a mut StepContext<'a>,
    text: &str,
    docstring: Option<&str>,
    table: Option<&[&[&str]]>,
) -> StepFuture<'a> {
    sync_to_async(failing_wrapper)(ctx, text, docstring, table)
}

step!(
    StepKeyword::Given,
    "fails",
    failing_wrapper,
    failing_wrapper_async,
    &[]
);

fn panicking_wrapper(
    ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let _ = ctx;
    catch_unwind(AssertUnwindSafe(|| panic!("snap"))).map_err(|e| StepError::PanicError {
        pattern: "panics".into(),
        function: "panicking_wrapper".into(),
        message: panic_message(e.as_ref()),
    })?;
    Ok(StepExecution::from_value(None))
}

fn panicking_wrapper_async<'a>(
    ctx: &'a mut StepContext<'a>,
    text: &str,
    docstring: Option<&str>,
    table: Option<&[&[&str]]>,
) -> StepFuture<'a> {
    sync_to_async(panicking_wrapper)(ctx, text, docstring, table)
}

step!(
    StepKeyword::When,
    "panics",
    panicking_wrapper,
    panicking_wrapper_async,
    &[]
);

fn needs_fixture_wrapper(
    ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    if ctx.get::<u32>("missing").is_some() {
        Ok(StepExecution::from_value(None))
    } else {
        Err(StepError::MissingFixture {
            name: "missing".into(),
            ty: "u32".into(),
            step: "needs_fixture".into(),
        })
    }
}

fn needs_fixture_wrapper_async<'a>(
    ctx: &'a mut StepContext<'a>,
    text: &str,
    docstring: Option<&str>,
    table: Option<&[&[&str]]>,
) -> StepFuture<'a> {
    sync_to_async(needs_fixture_wrapper)(ctx, text, docstring, table)
}

step!(
    StepKeyword::Then,
    "needs fixture",
    needs_fixture_wrapper,
    needs_fixture_wrapper_async,
    &["missing"]
);

// Test the 4-argument form (auto-generated async handler)
#[expect(
    clippy::unnecessary_wraps,
    reason = "wrapper must match StepFn signature"
)]
fn auto_async_wrapper(
    ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    let _ = ctx;
    Ok(StepExecution::from_value(None))
}

// Register using the 4-argument backward-compatible form
step!(
    StepKeyword::Given,
    "auto async step",
    auto_async_wrapper,
    &[]
);
