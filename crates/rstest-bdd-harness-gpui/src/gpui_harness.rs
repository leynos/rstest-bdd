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

    /// Runs a single GPUI scenario request, dispatching through `gpui::run_test`.
    ///
    /// The runner is taken from `runner_slot` exactly once. If the scenario
    /// function panics, the panic payload is augmented with feature context
    /// and re-raised via `panic::resume_unwind`. On success the result is
    /// stored in `output_slot` for later extraction.
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
                let (context, result) = result.unwrap_or_else(|payload| {
                    let message = Self::augmented_panic_message(payload.as_ref(), metadata);
                    Self::emit_augmented_panic_diagnostic(&message, metadata);
                    panic::resume_unwind(Box::new(message));
                });
                // A teardown panic should point at the GPUI cleanup path itself, not
                // at a scenario step that has already completed successfully.
                Self::finish_context(&dispatcher, &context);
                Self::store_output(output_slot, result);
            },
            None,
        );
    }

    /// Builds a [`TestAppContext`] and executes the scenario runner within it.
    ///
    /// Returns both the context and the runner output so the caller can
    /// perform post-scenario cleanup (quitting the context) separately from
    /// storing the result.
    fn run_scenario<T>(
        dispatcher: gpui::TestDispatcher,
        runner_slot: &Mutex<Option<ScenarioRunner<'_, TestAppContext, T>>>,
        scenario_name: &str,
    ) -> (TestAppContext, T) {
        let context = TestAppContext::build(dispatcher, None);
        let result = Self::run_with_runner(runner_slot, context.clone(), scenario_name);
        (context, result)
    }

    /// Takes the runner from `runner_slot` and invokes it with the given context.
    ///
    /// # Panics
    ///
    /// Panics if the runner has already been taken, which indicates the
    /// scenario was invoked more than once.
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

    /// Drains the dispatcher, forbids further parking, and quits the context.
    ///
    /// This must be called after every successful scenario run so that the
    /// GPUI event loop does not leak parked timers or background work into
    /// subsequent scenarios.
    fn finish_context(dispatcher: &gpui::TestDispatcher, context: &TestAppContext) {
        dispatcher.run_until_parked();
        context.executor().forbid_parking();
        context.quit();
        dispatcher.run_until_parked();
    }

    /// Stores the scenario result in `output_slot` for later extraction.
    fn store_output<T>(output_slot: &Mutex<Option<T>>, result: T) {
        *output_slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(result);
    }

    /// Extracts the scenario result from the output mutex.
    ///
    /// # Panics
    ///
    /// Panics if the output slot is still `None`, which indicates the GPUI
    /// test runner never produced a result.
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

    /// Emits the augmented panic message to both `tracing::error!` and stderr.
    ///
    /// This ensures the scenario context is visible in test logs even when
    /// the test runner does not collect tracing events.
    fn emit_augmented_panic_diagnostic(message: &str, metadata: &ScenarioMetadata) {
        tracing::error!(
            harness_type = "rstest_bdd_harness_gpui::GpuiHarness",
            feature_path = metadata.feature_path(),
            scenario_name = metadata.scenario_name(),
            scenario_line = metadata.scenario_line(),
            error = %message,
            "GPUI scenario panicked"
        );
        Self::write_stderr_diagnostic(message);
    }

    /// Builds an augmented panic message that includes the feature path,
    /// scenario name, and line number alongside the original panic payload text.
    pub fn augmented_panic_message(
        payload: &(dyn Any + Send),
        metadata: &ScenarioMetadata,
    ) -> String {
        let message = panic_message(payload);
        format!(
            "rstest-bdd-harness-gpui scenario panicked: feature={feature_path}:{scenario_line}, \
             scenario={scenario_name:?}: {message}",
            feature_path = metadata.feature_path(),
            scenario_line = metadata.scenario_line(),
            scenario_name = metadata.scenario_name(),
        )
    }

    /// Writes the diagnostic message to the locked stderr handle, logging
    /// any I/O failure at debug level rather than panicking.
    fn write_stderr_diagnostic(message: &str) {
        let mut stderr = io::stderr().lock();
        if let Err(error) = Self::write_stderr_diagnostic_to(&mut stderr, message) {
            tracing::debug!(
                harness_type = "rstest_bdd_harness_gpui::GpuiHarness",
                error = %error,
                "failed to write GPUI scenario panic diagnostic to stderr"
            );
        }
    }

    /// Writes the diagnostic message to an arbitrary [`Write`] sink.
    ///
    /// Visible for testing so callers can inject a failing writer and assert
    /// the function does not panic on I/O errors.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the underlying writer fails.
    pub fn write_stderr_diagnostic_to(writer: &mut impl Write, message: &str) -> io::Result<()> {
        writeln!(writer, "{message}")
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
