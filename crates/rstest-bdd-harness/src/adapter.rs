//! Harness adapter trait for scenario execution.

use crate::runner::ScenarioRunRequest;

/// Runs scenario closures inside a harness-specific environment.
///
/// # Examples
///
/// ```
/// use rstest_bdd_harness::{HarnessAdapter, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner, StdHarness};
///
/// let request = ScenarioRunRequest::new(
///     ScenarioMetadata::new("tests/features/demo.feature", "Example", 3, vec![]),
///     ScenarioRunner::new(|| 5 + 5),
/// );
/// let harness = StdHarness::new();
/// assert_eq!(harness.run(request), 10);
/// ```
pub trait HarnessAdapter {
    /// Executes one scenario request and returns the runner result.
    fn run<T>(&self, request: ScenarioRunRequest<'_, T>) -> T;
}
