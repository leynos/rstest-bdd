//! Harness adapter trait for scenario execution.

use crate::{HarnessResult, runner::ScenarioRunRequest};

/// Runs scenario closures inside a harness-specific environment.
///
/// # Examples
///
/// ```
/// use rstest_bdd_harness::{
///     HarnessAdapter, HarnessResult, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner,
///     StdHarness,
/// };
///
/// let request = ScenarioRunRequest::new(
///     ScenarioMetadata::new("tests/features/demo.feature", "Example", 3, vec![]),
///     ScenarioRunner::new_without_context(|| 5 + 5),
/// );
/// let harness = StdHarness::new();
/// let result: HarnessResult<i32> = harness.run(request);
/// assert_eq!(result.expect("std harness should not fail"), 10);
/// ```
pub trait HarnessAdapter {
    /// Harness-provided context passed into one scenario run.
    ///
    /// Harnesses that do not need to inject additional resources should use
    /// `()`.
    type Context: std::any::Any;

    /// Executes one scenario request and returns the runner result.
    ///
    /// # Errors
    ///
    /// Returns [`crate::HarnessError`] when the harness cannot initialize the
    /// environment required to execute the scenario.
    fn run<T>(&self, request: ScenarioRunRequest<'_, Self::Context, T>) -> HarnessResult<T>;
}
