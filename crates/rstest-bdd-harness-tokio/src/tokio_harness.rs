//! Tokio current-thread harness adapter for scenario execution.

use rstest_bdd_harness::{HarnessAdapter, ScenarioRunRequest};

/// Executes scenario runners inside a Tokio current-thread runtime with a
/// [`LocalSet`](tokio::task::LocalSet).
///
/// `TokioHarness` builds a new single-threaded Tokio runtime and a `LocalSet`
/// per scenario invocation, then blocks on the runner closure. The closure is
/// synchronous (`FnOnce() -> T`); the Tokio runtime and `LocalSet` are
/// active on the current thread so that `tokio::runtime::Handle::current()`,
/// `tokio::spawn`, and `tokio::task::spawn_local` are all available inside
/// step functions.
///
/// # Examples
///
/// ```
/// use rstest_bdd_harness::{
///     HarnessAdapter, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner,
/// };
/// use rstest_bdd_harness_tokio::TokioHarness;
///
/// let request = ScenarioRunRequest::new(
///     ScenarioMetadata::new(
///         "tests/features/demo.feature",
///         "Async scenario",
///         5,
///         vec![],
///     ),
///     ScenarioRunner::new(|| 2 + 2),
/// );
/// let harness = TokioHarness::new();
/// assert_eq!(harness.run(request), 4);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioHarness;

impl TokioHarness {
    /// Creates a new Tokio harness instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl HarnessAdapter for TokioHarness {
    fn run<T>(&self, request: ScenarioRunRequest<'_, T>) -> T {
        // FIXME(#443): propagate runtime build errors via Result once
        // HarnessAdapter::run returns Result<T, E>.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|err| {
                panic!("rstest-bdd-harness-tokio: failed to build Tokio runtime: {err}")
            });
        let local_set = tokio::task::LocalSet::new();
        local_set.block_on(&runtime, async {
            let result = request.run();
            // Yield once so that any tasks queued via `spawn_local` during
            // `request.run()` are driven to completion before we return.
            tokio::task::yield_now().await;
            result
        })
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the Tokio current-thread harness.

    use super::TokioHarness;
    use rstest::{fixture, rstest};
    use rstest_bdd_harness::{
        HarnessAdapter, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner,
    };

    #[fixture]
    fn harness() -> TokioHarness {
        TokioHarness::new()
    }

    #[rstest]
    fn tokio_harness_runs_request(harness: TokioHarness) {
        let request = ScenarioRunRequest::new(
            ScenarioMetadata::new(
                "tests/features/simple.feature",
                "Runs in Tokio",
                4,
                vec!["@async".to_string()],
            ),
            ScenarioRunner::new(|| 21 * 2),
        );
        assert_eq!(harness.run(request), 42);
    }

    #[rstest]
    fn tokio_runtime_is_active_during_run(harness: TokioHarness) {
        let request = ScenarioRunRequest::new(
            ScenarioMetadata::default(),
            ScenarioRunner::new(|| {
                // Panics if no Tokio runtime is active on the current thread.
                let _handle = tokio::runtime::Handle::current();
                true
            }),
        );
        assert!(harness.run(request));
    }
}
