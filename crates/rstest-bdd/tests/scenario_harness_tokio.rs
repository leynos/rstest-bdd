//! Integration tests verifying that `#[scenario]` works with the Tokio harness
//! adapter and attribute policy from `rstest-bdd-harness-tokio`.
//!
//! These tests prove end-to-end that:
//! - `TokioHarness` provides an active Tokio current-thread runtime during
//!   step execution.
//! - `TokioAttributePolicy` can be combined with `TokioHarness`.
//! - `spawn_local` succeeds in step functions, confirming the `LocalSet` +
//!   `current_thread` wiring.

use rstest_bdd_macros::{given, scenario, then, when};

#[given("the Tokio runtime is active")]
fn tokio_runtime_is_active() {
    // Panics if no Tokio runtime is active on the current thread.
    let _handle = tokio::runtime::Handle::current();
}

#[when("a Tokio handle is obtained")]
fn tokio_handle_is_obtained() {
    let _handle = tokio::runtime::Handle::current();
}

#[then("the handle confirms current-thread execution")]
fn handle_confirms_current_thread() {
    // `spawn_local` panics if no `LocalSet` context is active. Successfully
    // calling it proves the current-thread + LocalSet wiring provided by
    // TokioHarness. On a multi-threaded runtime without a `LocalSet`, this
    // would panic.
    tokio::task::spawn_local(async {});
}

/// Tests `#[scenario]` with `harness = TokioHarness` only.
#[scenario(
    path = "tests/features/tokio_harness.feature",
    name = "Tokio runtime is active during step execution",
    harness = rstest_bdd_harness_tokio::TokioHarness,
)]
fn scenario_runs_inside_tokio_runtime() {}

/// Tests `#[scenario]` with both `TokioHarness` and `TokioAttributePolicy`.
#[scenario(
    path = "tests/features/tokio_harness.feature",
    name = "Tokio harness with attribute policy",
    harness = rstest_bdd_harness_tokio::TokioHarness,
    attributes = rstest_bdd_harness_tokio::TokioAttributePolicy,
)]
fn scenario_runs_with_harness_and_policy() {}

// --- Async step definitions for TokioHarness ---

use std::sync::atomic::{AtomicBool, Ordering};

static ASYNC_GIVEN_RAN: AtomicBool = AtomicBool::new(false);
static ASYNC_WHEN_RAN: AtomicBool = AtomicBool::new(false);

#[given("an async given step runs")]
async fn async_given_step() {
    ASYNC_GIVEN_RAN.store(true, Ordering::Release);
}

#[when("an async when step runs")]
async fn async_when_step() {
    ASYNC_WHEN_RAN.store(true, Ordering::Release);
}

#[then("the async steps completed")]
async fn async_steps_completed() {
    assert!(
        ASYNC_GIVEN_RAN.load(Ordering::Acquire),
        "async given step should have executed"
    );
    assert!(
        ASYNC_WHEN_RAN.load(Ordering::Acquire),
        "async when step should have executed"
    );
}

/// Tests that `async fn` step definitions work with `TokioHarness`.
#[scenario(
    path = "tests/features/tokio_harness.feature",
    name = "Async step definitions execute under TokioHarness",
    harness = rstest_bdd_harness_tokio::TokioHarness,
)]
fn scenario_async_steps_under_tokio_harness() {}
