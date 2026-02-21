//! Tokio current-thread harness adapter for scenario execution.

use rstest_bdd_harness::{HarnessAdapter, ScenarioRunRequest};

/// Executes scenario runners inside a Tokio current-thread runtime.
///
/// `TokioHarness` builds a new single-threaded Tokio runtime per scenario
/// invocation and blocks on the runner closure. The closure is synchronous
/// (`FnOnce() -> T`); the Tokio runtime is available on the current thread
/// so that async step fallbacks and `tokio::spawn_local` work within the
/// scenario body.
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
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|err| {
                panic!("rstest-bdd-harness-tokio: failed to build Tokio runtime: {err}")
            });
        runtime.block_on(async { request.run() })
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the Tokio current-thread harness.

    use super::TokioHarness;
    use rstest_bdd_harness::{
        HarnessAdapter, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner,
    };

    #[test]
    fn tokio_harness_runs_request() {
        let request = ScenarioRunRequest::new(
            ScenarioMetadata::new(
                "tests/features/simple.feature",
                "Runs in Tokio",
                4,
                vec!["@async".to_string()],
            ),
            ScenarioRunner::new(|| 21 * 2),
        );
        let harness = TokioHarness::new();
        assert_eq!(harness.run(request), 42);
    }

    #[test]
    fn tokio_runtime_is_active_during_run() {
        let request = ScenarioRunRequest::new(
            ScenarioMetadata::default(),
            ScenarioRunner::new(|| {
                // Panics if no Tokio runtime is active on the current thread.
                let _handle = tokio::runtime::Handle::current();
                true
            }),
        );
        let harness = TokioHarness::new();
        assert!(harness.run(request));
    }
}
