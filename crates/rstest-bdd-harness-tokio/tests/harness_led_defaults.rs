//! Runtime integration tests for ADR-008 harness-led attribute-policy
//! defaults and their error paths.
//!
//! Unlike the snapshot and `RSTEST_BDD_RUN_MACROTEST`-gated expansion tests,
//! these run unconditionally under `cargo test` / `nextest` and assert
//! observable runtime behaviour:
//!
//! - `harness = TokioHarness` without `attributes = ...` runs through the
//!   inferred `TokioAttributePolicy` path with a live current-thread runtime
//!   and `LocalSet`.
//! - A harness whose `HarnessAdapter::run` returns `Err` propagates the
//!   `harness failed to initialise scenario: ...` panic emitted by the
//!   expanded macro, and the scenario body never runs.
//! - Pairing steps that rely on the harness contract with an attribute
//!   policy alone fails loudly (a `LocalSet` panic from `spawn_local`), not
//!   silently.

use rstest_bdd_harness::{HarnessAdapter, HarnessError, HarnessResult, ScenarioRunRequest};
use rstest_bdd_harness_tokio::TokioTestContext;
use rstest_bdd_macros::{given, scenario, then, when};

// --- Inferred-policy happy path -----------------------------------------

#[given("the inferred Tokio runtime is active")]
async fn inferred_runtime_is_active(
    #[from(rstest_bdd_harness_context)] context: &TokioTestContext,
) {
    // Panics if the inferred policy + harness did not stand up a runtime.
    assert_eq!(
        context.handle().runtime_flavor(),
        tokio::runtime::RuntimeFlavor::CurrentThread
    );
    std::future::ready(()).await;
}

#[when("a local task is spawned under the inferred policy")]
async fn local_task_spawned() {
    // Panics without the `LocalSet` provided by `TokioHarness`.
    tokio::task::spawn_local(async {});
    std::future::ready(()).await;
}

#[then("the inferred runtime flavour is current thread")]
async fn runtime_flavour_is_current_thread(
    #[from(rstest_bdd_harness_context)] context: &TokioTestContext,
) {
    std::future::ready(()).await;
    assert_eq!(
        context.handle().runtime_flavor(),
        tokio::runtime::RuntimeFlavor::CurrentThread
    );
}

/// `harness = TokioHarness` with no `attributes = ...`: the macro infers
/// `TokioAttributePolicy` (ADR-008) and the steps observe the live runtime.
#[scenario(
    path = "tests/features/harness_led_defaults.feature",
    name = "Inferred Tokio policy provides the runtime",
    harness = rstest_bdd_harness_tokio::TokioHarness,
)]
fn inferred_policy_runs_scenario_through_tokio_harness() {}

// --- Failing-harness error path ------------------------------------------

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

// --- Policy-without-harness mismatch -------------------------------------

#[given("a step that spawns a local task")]
fn step_spawning_local_task() {
    // `TokioAttributePolicy` filters `#[tokio::test]` away for sync test
    // functions, so no runtime or `LocalSet` exists here; `spawn_local`
    // panics, proving the mismatch is loud rather than silent.
    tokio::task::spawn_local(async {});
}

/// Pairing a harness-dependent step with the attribute policy alone (no
/// `harness = ...`) fails with a meaningful `LocalSet` panic rather than
/// silently passing.
#[scenario(
    path = "tests/features/harness_led_defaults.feature",
    name = "Attribute policy alone does not provide a LocalSet",
    attributes = rstest_bdd_harness_tokio::TokioAttributePolicy,
)]
#[should_panic(expected = "LocalSet")]
fn attribute_policy_without_harness_fails_loudly() {}
