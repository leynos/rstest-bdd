//! Unit tests for the GPUI harness adapter declared in
//! [`super::GpuiHarness`].
//!
//! This sibling submodule of `gpui_harness` carries privileged `use super`
//! access to the parent crate's private items, so it can exercise helpers
//! that are intentionally not part of the public API. Coverage is grouped
//! into two strands:
//!
//! - **Adapter behaviour:** [`gpui_harness_runs_request`] and
//!   [`gpui_test_context_is_available_during_run`] drive
//!   [`super::GpuiHarness::run`] end-to-end against `gpui::run_test`, so
//!   they take a `#[serial_test::serial]` guard to honour the GPUI
//!   single-thread requirement (see the parent module docs for the full
//!   constraint).
//! - **Panic-path helpers:**
//!   [`augmented_panic_message_includes_scenario_name_for_payload_type`]
//!   is an `rstest`-parametrised test that covers all three downcast arms
//!   of [`super::GpuiHarness::augmented_panic_message`] (owned `String`,
//!   `&'static str`, and an opaque `Debug`-only payload).
//!   [`write_stderr_diagnostic_to_returns_err_on_broken_pipe`] asserts
//!   that [`super::GpuiHarness::write_stderr_diagnostic_to`] surfaces an
//!   `Err` rather than panicking when the writer reports `BrokenPipe`.
//!   Neither helper test touches GPUI runtime state, so they run in
//!   parallel without serialisation.

/// An opaque panic-payload type that is neither `String` nor `&str`, used to
/// exercise the fallback downcast arm of `augmented_panic_message`.
#[derive(Debug)]
struct OpaquePayload;

use super::GpuiHarness;
use rstest::{fixture, rstest};
use rstest_bdd_harness::{HarnessAdapter, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner};

#[fixture]
fn harness() -> GpuiHarness {
    GpuiHarness::new()
}

#[rstest]
#[serial_test::serial]
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
#[serial_test::serial]
fn gpui_test_context_is_available_during_run(harness: GpuiHarness) {
    let request = ScenarioRunRequest::new(
        ScenarioMetadata::default(),
        ScenarioRunner::new(|context: gpui::TestAppContext| context.test_function_name().is_none()),
    );
    let result = harness
        .run(request)
        .unwrap_or_else(|err| panic!("gpui harness should not fail: {err}"));
    assert!(result);
}

/// Asserts that `augmented_panic_message` embeds the scenario name in the
/// returned string for all three panic-payload downcast paths:
///
/// - `string_payload`: the owned-`String` downcast arm.
/// - `str_payload`: the `&'static str` downcast arm.
/// - `opaque_payload`: the fallback arm for types that are neither.
#[rstest]
#[case::string_payload(
    "A string payload scenario",
    7,
    Box::new("step panicked".to_string()) as Box<dyn std::any::Any + Send>,
)]
#[case::str_payload(
    "A str payload scenario",
    12,
    Box::new("step panicked") as Box<dyn std::any::Any + Send>,
)]
#[case::opaque_payload(
    "An opaque payload scenario",
    17,
    Box::new(OpaquePayload) as Box<dyn std::any::Any + Send>,
)]
fn augmented_panic_message_includes_scenario_name_for_payload_type(
    #[case] scenario_name: &str,
    #[case] line: u32,
    #[case] payload: Box<dyn std::any::Any + Send>,
) {
    let metadata = ScenarioMetadata::new(
        "tests/features/example.feature",
        scenario_name,
        line,
        vec![],
    );
    let message = GpuiHarness::augmented_panic_message(payload.as_ref(), &metadata);
    assert!(
        message.contains(scenario_name),
        "expected scenario name in: {message}"
    );
}

/// Asserts that `write_stderr_diagnostic_to` returns an `Err` and does not
/// panic when the underlying writer always fails with `BrokenPipe`.
#[rstest]
fn write_stderr_diagnostic_to_returns_err_on_broken_pipe() {
    use std::io::{self, Write};

    /// A `Write` implementation that always returns a `BrokenPipe` error.
    struct BrokenPipeWriter;

    impl Write for BrokenPipeWriter {
        /// Always fails with `BrokenPipe` so the caller observes an `Err`
        /// instead of writing any bytes.
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }
        /// Succeeds without flushing because no bytes were written.
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let result = GpuiHarness::write_stderr_diagnostic_to(&mut BrokenPipeWriter, "test message");
    assert!(result.is_err(), "expected Err from broken writer, got Ok");
}
