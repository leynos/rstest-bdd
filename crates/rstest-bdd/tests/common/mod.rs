//! Common helper functions for behavioural tests.
//!
//! Not all test binaries use all helpers, so we expect unused code at the
//! module level rather than per-function.

// Not all test binaries use all helpers; each binary compiles this module
// separately, so some helpers may appear unused in certain binaries.
#![expect(
    dead_code,
    reason = "shared test helpers may be unused in some binaries"
)]

use rstest_bdd::{StepContext, StepError, StepExecution, StepFn, StepFuture};

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
/// This wrapper delegates to [`noop_wrapper`] and wraps the result in an
/// immediately-ready future.
///
/// # Examples
///
/// ```rust
/// use rstest_bdd::{StepContext, StepExecution, StepKeyword, step, find_step_async};
///
/// # mod common { include!("common/mod.rs"); }
/// # use common::{noop_wrapper, noop_async_wrapper, poll_step_future};
/// step!(StepKeyword::Given, "async example", noop_wrapper, noop_async_wrapper, &[]);
///
/// let async_fn = find_step_async(StepKeyword::Given, "async example".into()).unwrap();
/// let mut ctx = StepContext::default();
/// let future = async_fn(&mut ctx, "async example", None, None);
/// let result = poll_step_future(future);
/// assert!(matches!(result, StepExecution::Continue { .. }));
/// ```
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

/// Parameters for invoking a step function.
///
/// The explicit `'fixtures` lifetime is required so that async wrapper
/// signatures remain compatible with [`rstest_bdd::AsyncStepFn`], which
/// separates the lifetime of the borrowed [`StepContext`] from the lifetime of
/// the fixtures stored within it.
///
/// [`StepContext`]: rstest_bdd::StepContext
pub struct StepInvocationParams<'ctx, 'fixtures> {
    pub ctx: &'ctx mut StepContext<'fixtures>,
    pub text: &'ctx str,
    pub docstring: Option<&'ctx str>,
    pub table: Option<&'ctx [&'ctx [&'ctx str]]>,
}

/// Wrap a synchronous step function (`StepFn`) into an async wrapper.
///
/// This helper is used to build explicit async wrappers for sync steps in
/// integration tests.
pub fn wrap_sync_step_as_async<'ctx>(
    sync_fn: StepFn,
    params: StepInvocationParams<'ctx, '_>,
) -> StepFuture<'ctx> {
    let StepInvocationParams {
        ctx,
        text,
        docstring,
        table,
    } = params;
    Box::pin(std::future::ready(sync_fn(ctx, text, docstring, table)))
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
///
/// # mod common { include!("common/mod.rs"); }
/// # use common::poll_step_future;
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
