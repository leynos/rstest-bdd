//! Step wrapper definitions for registry tests.
//!
//! These wrappers are used by the step registry tests to exercise various
//! error conditions (failures, panics, missing fixtures) and the auto-generated
//! async handler feature.

use std::panic::{AssertUnwindSafe, catch_unwind};

use rstest_bdd::{
    StepContext, StepError, StepExecution, StepFuture, StepKeyword, panic_message, step,
};

use super::common::sync_to_async;

/// Generates an async wrapper function that delegates to a sync step function.
///
/// This eliminates boilerplate when registering sync steps that need explicit
/// async handlers for testing purposes.
///
/// # Usage
///
/// ```ignore
/// async_wrapper!(my_step_async, my_step);
/// ```
///
/// Expands to a function `my_step_async` that wraps `my_step` using `sync_to_async`.
macro_rules! async_wrapper {
    ($async_name:ident, $sync_fn:path) => {
        fn $async_name<'a>(
            ctx: &'a mut StepContext<'a>,
            text: &str,
            docstring: Option<&str>,
            table: Option<&[&[&str]]>,
        ) -> StepFuture<'a> {
            sync_to_async($sync_fn)(ctx, text, docstring, table)
        }
    };
}

/// Minimal step wrapper that succeeds without performing any action.
///
/// Used to verify that step registration and lookup work correctly.
#[expect(
    clippy::unnecessary_wraps,
    reason = "wrapper must match StepFn signature"
)]
fn wrapper(
    _ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    Ok(StepExecution::from_value(None))
}

async_wrapper!(wrapper_async, wrapper);

step!(
    StepKeyword::When,
    "behavioural",
    wrapper,
    wrapper_async,
    &[]
);

/// Step wrapper that always returns an `ExecutionError`.
///
/// Used to verify that step execution failures are correctly propagated
/// and reported by the test harness.
fn failing_wrapper(
    _ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    Err(StepError::ExecutionError {
        pattern: "fails".into(),
        function: "failing_wrapper".into(),
        message: "boom".into(),
    })
}

async_wrapper!(failing_wrapper_async, failing_wrapper);

step!(
    StepKeyword::Given,
    "fails",
    failing_wrapper,
    failing_wrapper_async,
    &[]
);

/// Step wrapper that triggers a panic and converts it to a `PanicError`.
///
/// Used to verify that panics during step execution are caught, converted
/// to errors, and correctly reported by the test harness.
fn panicking_wrapper(
    _ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    catch_unwind(AssertUnwindSafe(|| panic!("snap"))).map_err(|e| StepError::PanicError {
        pattern: "panics".into(),
        function: "panicking_wrapper".into(),
        message: panic_message(e.as_ref()),
    })?;
    Ok(StepExecution::from_value(None))
}

async_wrapper!(panicking_wrapper_async, panicking_wrapper);

step!(
    StepKeyword::When,
    "panics",
    panicking_wrapper,
    panicking_wrapper_async,
    &[]
);

/// Step wrapper that requires a fixture named "missing" of type `u32`.
///
/// Returns `MissingFixture` error when the fixture is not present, allowing
/// tests to verify that missing fixture errors are correctly detected and reported.
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

async_wrapper!(needs_fixture_wrapper_async, needs_fixture_wrapper);

step!(
    StepKeyword::Then,
    "needs fixture",
    needs_fixture_wrapper,
    needs_fixture_wrapper_async,
    &["missing"]
);

/// Step wrapper used to test the `step!` macro's auto-generation of async handlers.
///
/// When registered with the 4-argument form of `step!`, the macro automatically
/// generates an async wrapper, eliminating the need for an explicit async function.
#[expect(
    clippy::unnecessary_wraps,
    reason = "wrapper must match StepFn signature"
)]
fn auto_async_wrapper(
    _ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    Ok(StepExecution::from_value(None))
}

// Register using the backward-compatible form that auto-generates the async handler
step!(
    StepKeyword::Given,
    "auto async step",
    auto_async_wrapper,
    &[]
);
