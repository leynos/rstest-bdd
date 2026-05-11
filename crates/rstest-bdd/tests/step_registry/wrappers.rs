//! Step wrapper definitions for registry tests.
//!
//! These wrappers are used by the step registry tests to exercise various
//! error conditions (failures, panics, missing fixtures) and the auto-generated
//! async handler feature.

use std::panic::{AssertUnwindSafe, catch_unwind};

use rstest_bdd::{
    FixtureRequirement, RSTEST_BDD_HARNESS_CONTEXT_FIXTURE, StepContext, StepError, StepExecution,
    StepExecutionMode, StepFixtureRequirements, StepFuture, StepKeyword, StepPattern,
    panic_message, step, submit,
};

use super::common::{StepInvocationParams, wrap_sync_step_as_async};

/// Generates an async wrapper function that delegates to a sync step function.
///
/// This eliminates boilerplate when registering sync steps that need explicit
/// async handlers for testing purposes.
///
/// # Usage
///
/// ```ignore
/// async_wrapper!(my_step_async, my_step);
///
/// // Expands to:
/// // fn my_step_async<'ctx>(
/// //     ctx: &'ctx mut StepContext<'_>,
/// //     text: &'ctx str,
/// //     docstring: Option<&'ctx str>,
/// //     table: Option<&'ctx [&'ctx [&'ctx str]]>,
/// // ) -> StepFuture<'ctx> {
/// //     let params = StepInvocationParams {
/// //         ctx,
/// //         text,
/// //         docstring,
/// //         table,
/// //     };
/// //     wrap_sync_step_as_async(my_step, params)
/// // }
/// ```
macro_rules! async_wrapper {
    ($async_name:ident, $sync_fn:path) => {
        fn $async_name<'ctx>(
            ctx: &'ctx mut StepContext<'_>,
            text: &'ctx str,
            docstring: Option<&'ctx str>,
            table: Option<&'ctx [&'ctx [&'ctx str]]>,
        ) -> StepFuture<'ctx> {
            let params = StepInvocationParams {
                ctx,
                text,
                docstring,
                table,
            };
            wrap_sync_step_as_async($sync_fn, params)
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
    let panic_payload = catch_unwind(AssertUnwindSafe(|| panic!("snap")))
        .expect_err("closure unconditionally panics, so catch_unwind must return Err");
    Err(StepError::PanicError {
        pattern: "panics".into(),
        function: "panicking_wrapper".into(),
        message: panic_message(panic_payload.as_ref()),
    })
}

async_wrapper!(panicking_wrapper_async, panicking_wrapper);

step!(
    StepKeyword::When,
    "panics",
    panicking_wrapper,
    panicking_wrapper_async,
    &[]
);

/// Converts fixture presence into the wrapper result expected by registry tests.
fn fixture_present_or_error(
    found: bool,
    name: &'static str,
    ty: &'static str,
    step: &'static str,
) -> Result<StepExecution, StepError> {
    if found {
        Ok(StepExecution::from_value(None))
    } else {
        Err(StepError::MissingFixture {
            name: name.into(),
            ty: ty.into(),
            step: step.into(),
        })
    }
}

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
    fixture_present_or_error(
        ctx.get::<u32>("missing").is_some(),
        "missing",
        "u32",
        "needs_fixture",
    )
}

async_wrapper!(needs_fixture_wrapper_async, needs_fixture_wrapper);

step!(
    StepKeyword::Then,
    "needs fixture",
    needs_fixture_wrapper,
    needs_fixture_wrapper_async,
    &["missing"]
);

/// Step wrapper that requires the reserved harness context fixture.
fn needs_harness_context_wrapper(
    ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    fixture_present_or_error(
        ctx.borrow_ref::<u64>(RSTEST_BDD_HARNESS_CONTEXT_FIXTURE)
            .is_some(),
        RSTEST_BDD_HARNESS_CONTEXT_FIXTURE,
        "u64",
        "needs_harness_context",
    )
}

async_wrapper!(
    needs_harness_context_wrapper_async,
    needs_harness_context_wrapper
);

static NEEDS_HARNESS_CONTEXT_PATTERN: StepPattern = StepPattern::new("needs harness context");
static NEEDS_HARNESS_CONTEXT_REQUIREMENTS: [FixtureRequirement; 1] = [FixtureRequirement {
    name: RSTEST_BDD_HARNESS_CONTEXT_FIXTURE,
    ty: "u64",
}];

step!(
    @pattern
    StepKeyword::Then,
    &NEEDS_HARNESS_CONTEXT_PATTERN,
    needs_harness_context_wrapper,
    needs_harness_context_wrapper_async,
    &[RSTEST_BDD_HARNESS_CONTEXT_FIXTURE],
    StepExecutionMode::Both
);

submit! {
    StepFixtureRequirements {
        keyword: StepKeyword::Then,
        pattern: &NEEDS_HARNESS_CONTEXT_PATTERN,
        requirements: &NEEDS_HARNESS_CONTEXT_REQUIREMENTS,
    }
}

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

/// Step wrapper that succeeds and returns a value.
///
/// Used to test that `execute_step` correctly handles successful step execution
/// with a return value.
#[expect(
    clippy::unnecessary_wraps,
    reason = "wrapper must match StepFn signature"
)]
fn value_returning_wrapper(
    _ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    Ok(StepExecution::from_value(Some(Box::new(42i32))))
}

step!(
    StepKeyword::Given,
    "returns value",
    value_returning_wrapper,
    &[]
);

/// Helper to create a skip execution with an optional message.
///
/// This centralizes the skip construction logic to reduce duplication
/// between skip test wrappers.
fn create_skip_execution(message: Option<&str>) -> StepExecution {
    StepExecution::Skipped {
        message: message.map(String::from),
    }
}

/// Step wrapper that requests a skip without a message.
///
/// Used to test that `execute_step` correctly handles skip requests.
#[expect(
    clippy::unnecessary_wraps,
    reason = "wrapper must match StepFn signature"
)]
fn skip_wrapper(
    _ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    Ok(create_skip_execution(None))
}

step!(
    StepKeyword::When,
    "skips without message",
    skip_wrapper,
    &[]
);

/// Step wrapper that requests a skip with a message.
///
/// Used to test that `execute_step` correctly encodes and propagates skip messages.
#[expect(
    clippy::unnecessary_wraps,
    reason = "wrapper must match StepFn signature"
)]
fn skip_with_message_wrapper(
    _ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    Ok(create_skip_execution(Some("test skip reason")))
}

step!(
    StepKeyword::Then,
    "skips with message",
    skip_with_message_wrapper,
    &[]
);
