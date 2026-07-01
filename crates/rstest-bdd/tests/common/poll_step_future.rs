//! Step future polling helper for integration tests.

use rstest_bdd::{StepExecution, StepFuture};

/// Poll a step future to completion using a noop waker.
///
/// This helper encapsulates the boilerplate for polling an immediately-ready
/// future in tests. It returns the inner `StepExecution` on success.
///
/// # Panics
///
/// Panics if the future is not immediately ready or if it resolves to an error.
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
