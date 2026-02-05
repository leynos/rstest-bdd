//! Test support utilities for async step testing.
//!
//! This module provides helpers for testing step implementations across crates.
//! It is gated behind the `test-support` feature to avoid including polling
//! utilities in production builds.

use crate::{StepExecution, StepFuture};

pub use crate::async_step::sync_to_async;

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
///     _ctx: &'a mut StepContext<'_>,
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
