//! Harness adapter trait for scenario execution.

use crate::runner::ScenarioRunRequest;

/// Runs scenario closures inside a harness-specific environment.
///
/// # Examples
///
/// ```
/// use rstest_bdd_harness::{
///     HarnessAdapter, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner, StdHarness,
/// };
///
/// let request = ScenarioRunRequest::new(
///     ScenarioMetadata::new("tests/features/demo.feature", "Example", 3, vec![]),
///     ScenarioRunner::new_without_context(|| 5 + 5),
/// );
/// let harness = StdHarness::new();
/// assert_eq!(harness.run(request), 10);
/// ```
pub trait HarnessAdapter {
    /// Harness-provided context passed into one scenario run.
    ///
    /// Harnesses that do not need to inject additional resources should use
    /// `()`.
    type Context;

    /// Executes one scenario request and returns the runner result.
    fn run<T>(&self, request: ScenarioRunRequest<'_, Self::Context, T>) -> T;
}
