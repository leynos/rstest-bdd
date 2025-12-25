//! Common helper functions for behavioural tests.

use rstest_bdd::{StepContext, StepError, StepExecution, StepFuture};

/// No-op step wrapper matching the `StepFn` signature.
///
/// # Examples
/// ```rust
/// use rstest_bdd::{StepKeyword, step, find_step, StepContext};
///
/// # mod common { include!("common/mod.rs"); }
/// # use common::{noop_wrapper, noop_async_wrapper};
/// step!(StepKeyword::Given, "example", noop_wrapper, noop_async_wrapper, &[]);
/// let runner = find_step(StepKeyword::Given, "example".into()).unwrap();
/// let mut ctx = StepContext::default();
/// match runner(&mut ctx, "example", None, None) {
///     Ok(rstest_bdd::StepExecution::Continue { .. }) => {}
///     other => panic!("unexpected step outcome: {other:?}"),
/// }
/// ```
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
///
/// This wrapper delegates to `noop_wrapper` and wraps the result in an
/// immediately-ready future.
pub fn noop_async_wrapper<'a>(
    ctx: &'a mut StepContext<'a>,
    text: &str,
    docstring: Option<&str>,
    table: Option<&[&[&str]]>,
) -> StepFuture<'a> {
    Box::pin(std::future::ready(noop_wrapper(ctx, text, docstring, table)))
}
