//! Default synchronous harness implementation.

use crate::adapter::HarnessAdapter;
use crate::runner::ScenarioRunRequest;

/// Framework-agnostic synchronous harness.
///
/// `StdHarness` executes the scenario runner directly without an async runtime
/// or UI harness.
#[derive(Debug, Clone, Copy, Default)]
pub struct StdHarness;

impl StdHarness {
    /// Creates a new synchronous harness instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl HarnessAdapter for StdHarness {
    fn run<T>(&self, request: ScenarioRunRequest<'_, T>) -> T {
        request.run()
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the synchronous standard harness.

    use super::StdHarness;
    use crate::{HarnessAdapter, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner};

    #[test]
    fn std_harness_runs_request() {
        let request = ScenarioRunRequest::new(
            ScenarioMetadata::new(
                "tests/features/simple.feature",
                "Runs synchronously",
                4,
                vec!["@sync".to_string()],
            ),
            ScenarioRunner::new(|| 21 * 2),
        );
        let harness = StdHarness::new();
        assert_eq!(harness.run(request), 42);
    }
}
