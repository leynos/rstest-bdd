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
//!   `harness failed to initialize scenario: ...` panic emitted by the
//!   expanded macro, and the scenario body never runs.
//! - Pairing steps that rely on the harness contract with an attribute
//!   policy alone fails loudly (a `LocalSet` panic from `spawn_local`), not
//!   silently.

use std::sync::Arc;

use rstest_bdd_harness_tokio::TokioTestContext;
use rstest_bdd_macros::{given, scenario, then, when};
use tokio::sync::Notify;

include!("../../rstest-bdd-harness/tests/support/failing_harness_error_path.rs");

// --- Inferred-policy happy path -----------------------------------------

#[given("the inferred Tokio runtime is active")]
fn inferred_runtime_is_active(#[from(rstest_bdd_harness_context)] context: &TokioTestContext) {
    // Panics if the inferred policy + harness did not stand up a runtime.
    assert_eq!(
        context.handle().runtime_flavor(),
        tokio::runtime::RuntimeFlavor::CurrentThread
    );
}

#[when("a local task is spawned under the inferred policy")]
fn local_task_spawned() {
    // `spawn_local` panics without the `LocalSet` provided by `TokioHarness`.
    // This step is intentionally synchronous: the harness runner polls each
    // step future exactly once, so `.await` on the `JoinHandle` would yield
    // `Pending` and panic. Completion and value assertions are covered by the
    // standalone unit test `spawned_local_task_join_handle_returns_value`.
    let _handle = tokio::task::spawn_local(async { 42u32 });
}

#[then("the inferred runtime flavour is current thread")]
fn runtime_flavour_is_current_thread(
    #[from(rstest_bdd_harness_context)] context: &TokioTestContext,
) {
    assert_eq!(
        context.handle().runtime_flavor(),
        tokio::runtime::RuntimeFlavor::CurrentThread
    );
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

#[given("a step that must never run")]
fn step_that_must_never_run() {
    unreachable!("the failing harness must abort the scenario before steps run");
}

failing_harness_error_path_scenario!();

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
