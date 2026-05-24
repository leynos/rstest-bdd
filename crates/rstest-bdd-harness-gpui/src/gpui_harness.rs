//! GPUI harness adapter for scenario execution.
//!
//! When a step running under `GpuiHarness` panics, the harness captures the
//! panic payload, prepends the feature path, scenario name, and feature-file
//! line, then re-raises the augmented message via `panic::resume_unwind`. The
//! harness emits the same context to `tracing::error!` and to stderr so test
//! runners that do not collect tracing events still surface the scenario name
//! on failure.

use gpui::TestAppContext;
use rstest_bdd::panic_message;
use rstest_bdd_harness::{
    HarnessAdapter, HarnessResult, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner,
};
use std::any::Any;
use std::io::{self, Write};
use std::panic::{self, AssertUnwindSafe};
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
/// assert!(harness.run(request).expect("gpui harness should not fail"));
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
        metadata: &ScenarioMetadata,
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
                tracing::debug!(
                    harness_type = "rstest_bdd_harness_gpui::GpuiHarness",
                    feature_path = metadata.feature_path(),
                    scenario_name = metadata.scenario_name(),
                    scenario_line = metadata.scenario_line(),
                    "starting GPUI scenario"
                );
                let result = panic::catch_unwind(AssertUnwindSafe(|| {
                    Self::run_scenario(dispatcher.clone(), runner_slot, metadata.scenario_name())
                }));
                let (context, result) = result
                    .unwrap_or_else(|payload| Self::resume_augmented_panic(payload, metadata));
                // A teardown panic should point at the GPUI cleanup path itself, not
                // at a scenario step that has already completed successfully.
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

    fn resume_augmented_panic<T>(payload: Box<dyn Any + Send>, metadata: &ScenarioMetadata) -> T {
        let message = Self::augmented_panic_message(payload.as_ref(), metadata);
        drop(payload);
        tracing::error!(
            harness_type = "rstest_bdd_harness_gpui::GpuiHarness",
            feature_path = metadata.feature_path(),
            scenario_name = metadata.scenario_name(),
            scenario_line = metadata.scenario_line(),
            error = %message,
            "GPUI scenario panicked"
        );
        Self::write_stderr_diagnostic(&message);
        panic::resume_unwind(Box::new(message));
    }

    fn augmented_panic_message(payload: &(dyn Any + Send), metadata: &ScenarioMetadata) -> String {
        let message = panic_message(payload);
        format!(
            "rstest-bdd-harness-gpui scenario panicked: feature={feature_path}:{scenario_line}, \
             scenario={scenario_name:?}: {message}",
            feature_path = metadata.feature_path(),
            scenario_line = metadata.scenario_line(),
            scenario_name = metadata.scenario_name(),
        )
    }

    fn write_stderr_diagnostic(message: &str) {
        let mut stderr = io::stderr().lock();
        if let Err(error) = writeln!(stderr, "{message}") {
            tracing::debug!(
                harness_type = "rstest_bdd_harness_gpui::GpuiHarness",
                error = %error,
                "failed to write GPUI scenario panic diagnostic to stderr"
            );
        }
    }
}

impl HarnessAdapter for GpuiHarness {
    type Context = TestAppContext;

    fn run<T>(&self, request: ScenarioRunRequest<'_, Self::Context, T>) -> HarnessResult<T> {
        let (metadata, runner) = request.into_parts();
        let scenario_name = metadata.scenario_name().to_owned();
        let runner = Mutex::new(Some(runner));
        let output = Mutex::new(None);

        Self::run_request_once(&runner, &output, &metadata);
        Ok(Self::extract_output(output, &scenario_name))
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
        let result = harness
            .run(request)
            .unwrap_or_else(|err| panic!("gpui harness should not fail: {err}"));
        assert_eq!(result, 42);
    }

    #[rstest]
    fn gpui_test_context_is_available_during_run(harness: GpuiHarness) {
        let request = ScenarioRunRequest::new(
            ScenarioMetadata::default(),
            ScenarioRunner::new(|context: gpui::TestAppContext| {
                context.test_function_name().is_none()
            }),
        );
        let result = harness
            .run(request)
            .unwrap_or_else(|err| panic!("gpui harness should not fail: {err}"));
        assert!(result);
    }
}
