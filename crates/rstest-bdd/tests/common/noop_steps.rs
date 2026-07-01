//! No-op step wrappers for registry and diagnostic integration tests.

use rstest_bdd::{StepContext, StepError, StepExecution, StepFuture};

/// No-op step wrapper matching the `StepFn` signature.
///
/// # Errors
///
/// This helper never returns an error; the `Result` shape matches the `StepFn`
/// contract used by the registry macros.
#[expect(
    clippy::unnecessary_wraps,
    reason = "wrapper must match StepFn signature"
)]
pub fn noop_wrapper(
    ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    let _ = ctx;
    Ok(StepExecution::from_value(None))
}

/// Async version of the no-op step wrapper matching the `AsyncStepFn` signature.
pub fn noop_async_wrapper<'ctx>(
    ctx: &'ctx mut StepContext<'_>,
    text: &'ctx str,
    docstring: Option<&'ctx str>,
    table: Option<&'ctx [&'ctx [&'ctx str]]>,
) -> StepFuture<'ctx> {
    Box::pin(std::future::ready(noop_wrapper(
        ctx, text, docstring, table,
    )))
}
