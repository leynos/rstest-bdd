//! Default synchronous harness implementation.

use crate::adapter::HarnessAdapter;
use crate::runner::ScenarioRunRequest;

/// Framework-agnostic synchronous harness.
///
/// `StdHarness` executes the scenario runner directly without an async runtime
/// or UI harness.
pub struct StdHarness;

impl HarnessAdapter for StdHarness {
    fn run<T>(request: ScenarioRunRequest<T>) -> T {
        request.run()
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!(StdHarness::run(request), 42);
    }
}
