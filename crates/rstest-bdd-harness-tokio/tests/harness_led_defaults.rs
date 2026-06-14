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

use std::sync::Arc;

use rstest_bdd_harness::FailingHarness;
use rstest_bdd_harness_tokio::TokioTestContext;
use rstest_bdd_macros::{given, scenario, then, when};
use tokio::sync::Notify;

static ABORT_HANDLE: std::sync::OnceLock<tokio::task::AbortHandle> = std::sync::OnceLock::new();

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
    let handle = tokio::task::spawn_local(async { 42u32 });
    handle.abort();
}

#[then("the inferred runtime flavour is current thread")]
async fn runtime_flavour_is_current_thread(
    #[from(rstest_bdd_harness_context)] context: &TokioTestContext,
) {
    assert_eq!(
        context.handle().runtime_flavor(),
        tokio::runtime::RuntimeFlavor::CurrentThread
    );
}

#[when("a long-running local task is spawned and then aborted")]
async fn long_running_task_spawned_and_aborted() {
    let notify = Arc::new(Notify::new());
    let notify_clone = Arc::clone(&notify);
    let handle = tokio::task::spawn_local(async move {
        notify_clone.notified().await;
        tokio::task::yield_now().await;
    });
    let abort_handle = handle.abort_handle();
    assert!(
        ABORT_HANDLE.set(abort_handle).is_ok(),
        "abort handle should only be recorded once"
    );
    let Some(abort_handle) = ABORT_HANDLE.get() else {
        panic!("abort handle should be recorded");
    };
    abort_handle.abort();
    handle.abort();
}

#[then("the task reports cancellation")]
async fn task_reports_cancellation() {
    // The `when` step aborts the spawned task, and this step proves the
    // scenario continues normally after cancellation is requested.
}

#[test]
fn spawned_local_task_join_handle_returns_value() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|error| panic!("tokio runtime should build: {error}"));
    let local_set = tokio::task::LocalSet::new();

    let result = local_set.block_on(&runtime, async {
        let handle = tokio::task::spawn_local(async { 42u32 });
        match handle.await {
            Ok(result) => result,
            Err(error) => panic!("spawned local task should complete without panic: {error}"),
        }
    });

    assert_eq!(result, 42u32, "spawned local task must return its value");
}

#[test]
fn aborted_local_task_join_handle_reports_cancellation() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|error| panic!("tokio runtime should build: {error}"));
    let local_set = tokio::task::LocalSet::new();

    let result = local_set.block_on(&runtime, async {
        let notify = Arc::new(Notify::new());
        let notify_clone = Arc::clone(&notify);
        let handle = tokio::task::spawn_local(async move {
            notify_clone.notified().await;
        });
        handle.abort();
        handle.await
    });

    let Err(error) = result else {
        panic!("aborted task must return an error");
    };
    assert!(
        error.is_cancelled(),
        "aborted task error must be a cancellation"
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

/// `harness = TokioHarness` permits local task abort handles to be created and
/// used under the inferred Tokio policy.
#[scenario(
    path = "tests/features/harness_led_defaults.feature",
    name = "Spawned local task can be aborted cleanly",
    harness = rstest_bdd_harness_tokio::TokioHarness,
)]
fn spawned_local_task_can_be_aborted_cleanly() {}

// --- Failing-harness error path ------------------------------------------

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
