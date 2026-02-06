//! Public helpers for explicit async step wrappers.
//!
//! This module provides stable, non-test-specific helpers for users who need
//! to write explicit async wrapper functions, while preserving the crate's
//! two-lifetime async model.

use crate::{StepContext, StepFn, StepFuture};

/// Wrap a synchronous step handler into an immediately-ready async future.
///
/// This helper uses currying to reduce the parameter count: it takes the
/// sync function and returns a closure with the same arguments and return type
/// as `AsyncStepFn`.
///
/// # Examples
///
/// ```rust
/// use rstest_bdd::async_step::sync_to_async;
/// use rstest_bdd::{StepContext, StepError, StepExecution, StepFuture};
///
/// fn my_sync_step(
///     _ctx: &mut StepContext<'_>,
///     _text: &str,
///     _docstring: Option<&str>,
///     _table: Option<&[&[&str]]>,
/// ) -> Result<StepExecution, StepError> {
///     Ok(StepExecution::from_value(None))
/// }
///
/// fn my_async_step<'ctx>(
///     ctx: &'ctx mut StepContext<'_>,
///     text: &'ctx str,
///     docstring: Option<&'ctx str>,
///     table: Option<&'ctx [&'ctx [&'ctx str]]>,
/// ) -> StepFuture<'ctx> {
///     sync_to_async(my_sync_step)(ctx, text, docstring, table)
/// }
/// ```
#[expect(
    clippy::type_complexity,
    reason = "currying captures StepFn, so this helper cannot return the AsyncStepFn fn-pointer alias"
)]
pub fn sync_to_async(
    sync_fn: StepFn,
) -> impl for<'ctx, 'fixtures> FnOnce(
    &'ctx mut StepContext<'fixtures>,
    &'ctx str,
    Option<&'ctx str>,
    Option<&'ctx [&'ctx [&'ctx str]]>,
) -> StepFuture<'ctx> {
    move |ctx, text, docstring, table| {
        Box::pin(std::future::ready(sync_fn(ctx, text, docstring, table)))
    }
}
