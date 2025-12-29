//! Test support utilities for async step testing.
//!
//! This module provides helpers for testing step implementations across crates.
//! It is gated behind the `test-support` feature to avoid including test
//! utilities in production builds.

use crate::{StepContext, StepError, StepExecution, StepFuture};

/// Wrap a synchronous step handler into an immediately-ready async future.
///
/// This helper uses currying to reduce the parameter count: it takes the
/// sync function and returns a closure matching the `AsyncStepFn` signature.
///
/// # Examples
///
/// ```rust
/// use rstest_bdd::{StepContext, StepError, StepExecution, StepFuture};
/// use rstest_bdd::test_support::sync_to_async;
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
/// fn my_async_step<'a>(
///     ctx: &'a mut StepContext<'a>,
///     text: &str,
///     docstring: Option<&str>,
///     table: Option<&[&[&str]]>,
/// ) -> StepFuture<'a> {
///     sync_to_async(my_sync_step)(ctx, text, docstring, table)
/// }
/// ```
#[expect(
    clippy::type_complexity,
    reason = "currying pattern produces complex return type to reduce parameter count"
)]
pub fn sync_to_async<'a, F>(
    sync_fn: F,
) -> impl FnOnce(&'a mut StepContext<'a>, &str, Option<&str>, Option<&[&[&str]]>) -> StepFuture<'a>
where
    F: FnOnce(
            &mut StepContext<'_>,
            &str,
            Option<&str>,
            Option<&[&[&str]]>,
        ) -> Result<StepExecution, StepError>
        + 'a,
{
    move |ctx, text, docstring, table| {
        Box::pin(std::future::ready(sync_fn(ctx, text, docstring, table)))
    }
}

/// Poll a step future to completion using a noop waker.
///
/// This helper encapsulates the boilerplate for polling an immediately-ready
/// future in tests. It returns the inner `StepExecution` on success.
///
/// # Panics
///
/// Panics if the future is not immediately ready or if it resolves to an error.
///
/// # Examples
///
/// ```rust
/// use rstest_bdd::{StepContext, StepExecution, StepFuture};
/// use rstest_bdd::test_support::poll_step_future;
///
/// fn example_async<'a>(
///     _ctx: &'a mut StepContext<'a>,
///     _text: &str,
///     _docstring: Option<&str>,
///     _table: Option<&[&[&str]]>,
/// ) -> StepFuture<'a> {
///     Box::pin(std::future::ready(Ok(StepExecution::from_value(None))))
/// }
///
/// let mut ctx = StepContext::default();
/// let future = example_async(&mut ctx, "test", None, None);
/// let result = poll_step_future(future);
/// assert!(matches!(result, StepExecution::Continue { .. }));
/// ```
pub fn poll_step_future(future: StepFuture<'_>) -> StepExecution {
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(waker);
    let mut pinned = future;
    match std::pin::Pin::as_mut(&mut pinned).poll(&mut cx) {
        std::task::Poll::Ready(Ok(execution)) => execution,
        std::task::Poll::Ready(Err(e)) => panic!("step future resolved to error: {e:?}"),
        std::task::Poll::Pending => panic!("step future was not immediately ready"),
    }
}
