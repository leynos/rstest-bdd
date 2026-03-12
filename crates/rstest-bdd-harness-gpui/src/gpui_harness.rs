//! GPUI harness adapter for scenario execution.

use gpui::TestAppContext;
use rstest_bdd_harness::{HarnessAdapter, ScenarioRunRequest, ScenarioRunner};
use std::sync::{Mutex, PoisonError};

/// Executes scenario runners inside the GPUI test harness.
///
/// `GpuiHarness` uses `gpui::run_test` with one iteration and no retries,
/// then builds a `TestAppContext` for the scenario and passes it through
/// `request.run(context)`. Step functions can request the context through the
/// reserved fixture key `rstest_bdd_harness_context`.
///
/// # Examples
///
/// ```
/// use rstest_bdd_harness::{HarnessAdapter, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner};
/// use rstest_bdd_harness_gpui::GpuiHarness;
///
/// let request = ScenarioRunRequest::new(
///     ScenarioMetadata::new(
///         "tests/features/demo.feature",
///         "GPUI scenario",
///         5,
///         vec![],
///     ),
///     ScenarioRunner::new(|cx: gpui::TestAppContext| cx.test_function_name().is_none()),
/// );
///
/// let harness = GpuiHarness::new();
/// assert!(harness.run(request));
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct GpuiHarness;

impl GpuiHarness {
    /// Creates a new GPUI harness instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    fn run_request_once<T>(
        runner_slot: &Mutex<Option<ScenarioRunner<'_, TestAppContext, T>>>,
        output_slot: &Mutex<Option<T>>,
        scenario_name: &str,
    ) {
        gpui::run_test(
            1,
            &[],
            0,
            &mut |dispatcher, _seed| {
                if output_slot
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .is_some()
                {
                    return;
                }
                let (context, result) =
                    Self::run_scenario(dispatcher.clone(), runner_slot, scenario_name);
                Self::finish_context(&dispatcher, &context);
                Self::store_output(output_slot, result);
            },
            None,
        );
    }

    fn run_scenario<T>(
        dispatcher: gpui::TestDispatcher,
        runner_slot: &Mutex<Option<ScenarioRunner<'_, TestAppContext, T>>>,
        scenario_name: &str,
    ) -> (TestAppContext, T) {
        let context = TestAppContext::build(dispatcher, None);
        let result = Self::run_with_runner(runner_slot, context.clone(), scenario_name);
        (context, result)
    }

    fn run_with_runner<T>(
        runner_slot: &Mutex<Option<ScenarioRunner<'_, TestAppContext, T>>>,
        context: TestAppContext,
        scenario_name: &str,
    ) -> T {
        runner_slot
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .unwrap_or_else(|| {
                panic!(
                    "rstest-bdd-harness-gpui: scenario runner invoked more than once: \
                     {scenario_name}"
                )
            })
            .run(context)
    }

    fn finish_context(dispatcher: &gpui::TestDispatcher, context: &TestAppContext) {
        dispatcher.run_until_parked();
        context.executor().forbid_parking();
        context.quit();
        dispatcher.run_until_parked();
    }

    fn store_output<T>(output_slot: &Mutex<Option<T>>, result: T) {
        *output_slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(result);
    }

    fn extract_output<T>(output: Mutex<Option<T>>, scenario_name: &str) -> T {
        output
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner)
            .unwrap_or_else(|| {
                panic!(
                    "rstest-bdd-harness-gpui: test harness produced no scenario result: \
                     {scenario_name}"
                )
            })
    }
}

impl HarnessAdapter for GpuiHarness {
    type Context = TestAppContext;

    fn run<T>(&self, request: ScenarioRunRequest<'_, Self::Context, T>) -> T {
        let (metadata, runner) = request.into_parts();
        let scenario_name = metadata.scenario_name().to_owned();
        let runner = Mutex::new(Some(runner));
        let output = Mutex::new(None);

        Self::run_request_once(&runner, &output, &scenario_name);
        Self::extract_output(output, &scenario_name)
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the GPUI harness adapter.

    use super::GpuiHarness;
    use rstest::{fixture, rstest};
    use rstest_bdd_harness::{
        HarnessAdapter, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner,
    };

    #[fixture]
    fn harness() -> GpuiHarness {
        GpuiHarness::new()
    }

    #[rstest]
    fn gpui_harness_runs_request(harness: GpuiHarness) {
        let request = ScenarioRunRequest::new(
            ScenarioMetadata::new(
                "tests/features/simple.feature",
                "Runs in GPUI",
                4,
                vec!["@ui".to_string()],
            ),
            ScenarioRunner::new(|_context: gpui::TestAppContext| 21 * 2),
        );
        assert_eq!(harness.run(request), 42);
    }

    #[rstest]
    fn gpui_test_context_is_available_during_run(harness: GpuiHarness) {
        let request = ScenarioRunRequest::new(
            ScenarioMetadata::default(),
            ScenarioRunner::new(|context: gpui::TestAppContext| {
                context.test_function_name().is_none()
            }),
        );
        assert!(harness.run(request));
    }
}
