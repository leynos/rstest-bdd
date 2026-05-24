//! Per-scenario context injected by [`crate::TokioHarness`].

use tokio::runtime::Handle;

/// Context injected by [`crate::TokioHarness`] into each scenario run.
///
/// Step functions may request this context via the reserved fixture key
/// `rstest_bdd_harness_context`.
///
/// # Examples
///
/// ```
/// use rstest_bdd_macros::given;
/// use rstest_bdd_harness_tokio::TokioTestContext;
///
/// #[given("a background task is spawned")]
/// async fn spawn_background_task(
///     #[from(rstest_bdd_harness_context)] context: &TokioTestContext,
/// ) {
///     let task = context.handle().spawn(async { 2 + 2 });
///     assert_eq!(task.await.expect("background task should complete"), 4);
/// }
/// ```
#[derive(Clone)]
pub struct TokioTestContext {
    handle: Handle,
}

impl TokioTestContext {
    /// Captures the currently-active Tokio runtime handle.
    ///
    /// # Panics
    ///
    /// Panics when called outside an active Tokio runtime.
    #[must_use]
    pub fn from_current() -> Self {
        Self {
            handle: Handle::current(),
        }
    }

    /// Returns a reference to the Tokio runtime handle.
    #[must_use]
    pub const fn handle(&self) -> &Handle {
        &self.handle
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for Tokio test context capture.

    use super::TokioTestContext;

    #[tokio::test(flavor = "current_thread")]
    async fn from_current_captures_handle_that_can_spawn() {
        let context = TokioTestContext::from_current();
        let task = context.handle().spawn(async { 21 * 2 });
        let result = task.await.map_err(|err| err.to_string());

        assert_eq!(result, Ok(42));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cloned_context_can_spawn() {
        let context = TokioTestContext::from_current();
        let cloned = context.clone();
        let task = cloned.handle().spawn(async { 10 + 5 });
        let result = task.await.map_err(|err| err.to_string());

        assert_eq!(result, Ok(15));
    }

    #[test]
    fn from_current_panics_outside_runtime() {
        let result = std::panic::catch_unwind(TokioTestContext::from_current);

        assert!(
            result.is_err(),
            "capturing a Tokio context outside a runtime should panic"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handle_returns_stable_reference() {
        let context = TokioTestContext::from_current();

        assert!(std::ptr::eq(context.handle(), context.handle()));
    }
}
