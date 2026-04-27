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
/// let feature_path = request.metadata().feature_path().to_string();
/// let scenario_name = request.metadata().scenario_name().to_string();
/// let result: HarnessResult<i32> = harness.run(request).map_err(|err| {
///     let err = err.with_scenario_context(feature_path.clone(), scenario_name.clone());
///     tracing::error!(
///         %harness_type = std::any::type_name::<StdHarness>(),
///         %feature_path,
///         %scenario_name,
///         %err,
///         "harness failed to initialize scenario"
///     );
///     err.into_error()
/// });
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
