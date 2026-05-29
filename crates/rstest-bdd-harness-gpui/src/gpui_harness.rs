//! GPUI harness adapter for scenario execution.
//!
//! When a step running under `GpuiHarness` panics, the harness captures the
//! panic payload, prepends the feature path, scenario name, and feature-file
//! line, then re-raises the augmented message via `panic::resume_unwind`. The
//! harness records the same context as a `tracing::error!` event and writes
//! the augmented diagnostic to stderr so test runners that do not collect
//! tracing events still surface the scenario name on failure.
//!
//! Per-scenario GPUI cleanup runs through a [`ContextCleanup`] RAII guard so
//! `finish_context` executes on both the success and the panic paths,
//! preventing parked timers or background work from leaking into subsequent
//! scenarios.

use gpui::TestAppContext;
use rstest_bdd::panic_message;
use rstest_bdd_harness::{
    HarnessAdapter, HarnessResult, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner,
};
use std::any::Any;
use std::cell::RefCell;
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

/// RAII guard that invokes [`GpuiHarness::finish_context`] when dropped.
///
/// Constructing the guard immediately after a scenario's [`TestAppContext`]
/// is built ensures the GPUI cleanup contract is honoured whether the
/// scenario returns normally or panics: on the success path the guard drops
/// at the end of the closure body; on the panic path it drops while the
/// stack unwinds toward `gpui::run_test`. Either way the dispatcher is run
/// to quiescence, parking is forbidden, and the context is quit, so the
/// next scenario starts from a clean GPUI event loop.
struct ContextCleanup<'a> {
    dispatcher: &'a gpui::TestDispatcher,
    context: &'a TestAppContext,
}

impl Drop for ContextCleanup<'_> {
    fn drop(&mut self) {
        GpuiHarness::finish_context(self.dispatcher, self.context);
    }
}

impl GpuiHarness {
    /// Creates a new GPUI harness instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Runs a single GPUI scenario request, dispatching through `gpui::run_test`.
    ///
    /// The runner is taken from `runner_slot` exactly once. The scenario's
    /// [`TestAppContext`] is built before the runner is invoked so a
    /// [`ContextCleanup`] guard can ensure `finish_context` runs on both the
    /// success and the panic paths.
    ///
    /// The caller supplies `stderr_writer`, which receives the scenario
    /// diagnostic when the step panics.  Selecting stderr (typically
    /// `io::stderr().lock()`) is the caller's responsibility; the function
    /// treats the writer opaquely and does not open any I/O sink on its own.
    ///
    /// If the scenario function panics, the panic payload is augmented with
    /// feature context, recorded as a `tracing::error!` event, written to
    /// `stderr_writer`, and re-raised via `panic::resume_unwind`. On success
    /// the result is stored in `output_slot` for later extraction.
    fn run_request_once<T, W: Write>(
        runner_slot: &Mutex<Option<ScenarioRunner<'_, TestAppContext, T>>>,
        output_slot: &Mutex<Option<T>>,
        metadata: &ScenarioMetadata,
        stderr_writer: &AssertUnwindSafe<RefCell<W>>,
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

                let context = TestAppContext::build(dispatcher.clone(), None);
                let _cleanup = ContextCleanup {
                    dispatcher: &dispatcher,
                    context: &context,
                };

                let runner_result = panic::catch_unwind(AssertUnwindSafe(|| {
                    Self::run_with_runner(runner_slot, context.clone(), metadata.scenario_name())
                }));

                match runner_result {
                    Ok(value) => Self::store_output(output_slot, value),
                    Err(payload) => {
                        let message = Self::augmented_panic_message(payload.as_ref(), metadata);
                        Self::record_and_write_panic_diagnostic(
                            &message,
                            metadata,
                            &mut *stderr_writer.borrow_mut(),
                        );
                        // The original payload has been transcribed into `message`. Leak
                        // it so an arbitrary `Drop` impl on the boxed `Any` cannot panic
                        // during the new unwind and trigger a double-panic abort.
                        std::mem::forget(payload);
                        panic::resume_unwind(Box::new(message));
                    }
                }
            },
            None,
        );
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
    /// This must be called after every scenario run, on both the success and
    /// the panic paths, so the GPUI event loop does not leak parked timers
    /// or background work into subsequent scenarios. The [`ContextCleanup`]
    /// guard enforces this contract from within [`run_request_once`].
    ///
    /// [`run_request_once`]: GpuiHarness::run_request_once
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

    /// Records the augmented panic message as a structured `tracing::error!`
    /// event so subscribers can filter and correlate scenario failures by
    /// harness, feature, and scenario fields.
    ///
    /// This function performs observability only: it does not touch stderr
    /// or any other I/O sink. The caller is responsible for injecting an
    /// explicit writer (typically [`io::stderr`]) when stderr surfacing is
    /// also wanted.
    fn record_panic_event(message: &str, metadata: &ScenarioMetadata) {
        tracing::error!(
            harness_type = "rstest_bdd_harness_gpui::GpuiHarness",
            feature_path = metadata.feature_path(),
            scenario_name = metadata.scenario_name(),
            scenario_line = metadata.scenario_line(),
            error = %message,
            "GPUI scenario panicked"
        );
    }

    /// Records the augmented diagnostic as a `tracing::error!` event and writes
    /// it to `writer`.  Write failures are downgraded to a `tracing::debug!`
    /// event; they never propagate as errors or panics so the caller's unwind
    /// path is unaffected.
    fn record_and_write_panic_diagnostic(
        message: &str,
        metadata: &ScenarioMetadata,
        writer: &mut impl Write,
    ) {
        Self::record_panic_event(message, metadata);
        if let Err(error) = Self::write_stderr_diagnostic_to(writer, message) {
            tracing::debug!(
                harness_type = "rstest_bdd_harness_gpui::GpuiHarness",
                feature_path = metadata.feature_path(),
                scenario_name = metadata.scenario_name(),
                scenario_line = metadata.scenario_line(),
                error = %error,
                "failed to write GPUI scenario panic diagnostic to stderr"
            );
        }
    }

    /// Builds an augmented panic message that includes the feature path,
    /// scenario name, and line number alongside the original panic payload text.
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

    /// Writes the diagnostic message to an arbitrary [`Write`] sink.
    ///
    /// This is the injectable I/O primitive used by [`run_request_once`].
    /// Callers select the writer explicitly (typically `io::stderr().lock()`)
    /// and decide how to handle failures, keeping side-effects visible at
    /// the call site rather than hidden behind a no-argument wrapper.
    ///
    /// [`run_request_once`]: GpuiHarness::run_request_once
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the underlying writer fails.
    fn write_stderr_diagnostic_to(writer: &mut impl Write, message: &str) -> io::Result<()> {
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

        let stderr = io::stderr().lock();
        let stderr_writer = AssertUnwindSafe(RefCell::new(stderr));
        Self::run_request_once(&runner, &output, &metadata, &stderr_writer);
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
