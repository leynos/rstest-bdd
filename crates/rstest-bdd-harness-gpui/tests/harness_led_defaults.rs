//! Runtime integration tests for ADR-008 harness-led attribute-policy
//! defaults and their error paths in the GPUI harness crate.
//!
//! Unlike the snapshot and `RSTEST_BDD_RUN_MACROTEST`-gated expansion tests,
//! these run unconditionally under `cargo test` / `nextest` and assert
//! observable runtime behaviour:
//!
//! - A harness whose `HarnessAdapter::run` returns `Err` propagates the
//!   `harness failed to initialise scenario: ...` panic emitted by the
//!   expanded macro, and the scenario body never runs. This path needs no
//!   native GPUI runtime, so it is not feature-gated.
//! - `harness = GpuiHarness` without `attributes = ...` runs through the
//!   inferred `GpuiAttributePolicy` path with a live `TestAppContext`. This
//!   requires the native GPUI test runtime, so it shares the
//!   `native-gpui-tests` gate (and `#[serial]` discipline) with the rest of
//!   the GPUI scenario suite.

use rstest_bdd_harness::{HarnessAdapter, HarnessError, HarnessResult, ScenarioRunRequest};
use rstest_bdd_macros::{given, scenario};

// --- Failing-harness error path (no native GPUI runtime required) --------

/// Harness whose `run` always fails before invoking the scenario runner.
#[derive(Default)]
struct FailingHarness;

impl HarnessAdapter for FailingHarness {
    type Context = ();

    fn run<T>(&self, _request: ScenarioRunRequest<'_, Self::Context, T>) -> HarnessResult<T> {
        Err(HarnessError::RuntimeBuildFailed(std::io::Error::other(
            "synthetic harness initialisation failure",
        )))
    }
}

#[given("a step that must never run")]
fn step_that_must_never_run() {
    unreachable!("the failing harness must abort the scenario before steps run");
}

/// A harness `run` returning `Err` must surface the macro's
/// `harness failed to initialise scenario: ...` panic, carrying the
/// underlying error and scenario context, and must not execute any step.
#[scenario(
    path = "tests/features/harness_led_defaults.feature",
    name = "Failing harness initialisation propagates",
    harness = FailingHarness,
)]
#[should_panic(expected = "harness failed to initialise scenario: failed to build runtime")]
fn failing_harness_panics_with_meaningful_message() {}

// --- Inferred-policy happy path (requires the native GPUI runtime) -------

#[cfg(feature = "native-gpui-tests")]
mod native {
    //! Inferred-policy coverage that drives the real GPUI test runtime.

    use rstest_bdd_macros::{given, scenario};
    use serial_test::serial;

    #[given("the inferred GPUI context is observed")]
    fn inferred_gpui_context_is_observed(
        #[from(rstest_bdd_harness_context)] context: &gpui::TestAppContext,
    ) {
        // Receiving the reserved harness-context fixture proves the
        // inferred policy + harness pairing injected the GPUI context.
        assert!(context.test_function_name().is_none());
    }

    /// `harness = GpuiHarness` with no `attributes = ...`: the macro infers
    /// `GpuiAttributePolicy` (ADR-008) and the step observes the injected
    /// `TestAppContext` at runtime.
    #[scenario(
        path = "tests/features/harness_led_defaults.feature",
        name = "Inferred GPUI policy provides the test context",
        harness = rstest_bdd_harness_gpui::GpuiHarness,
    )]
    #[serial]
    fn inferred_policy_runs_scenario_through_gpui_harness() {}
}
