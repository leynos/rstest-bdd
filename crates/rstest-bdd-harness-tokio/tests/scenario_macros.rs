//! Integration tests verifying that `#[scenario]` works with this crate's
//! Tokio harness adapter and attribute policy.
//!
//! These tests prove end-to-end that:
//! - `TokioHarness` provides an active Tokio current-thread runtime during
//!   step execution.
//! - `TokioAttributePolicy` can be combined with `TokioHarness`.
//! - `spawn_local` succeeds in step functions, confirming the `LocalSet` +
//!   `current_thread` wiring.

use rstest_bdd_harness_tokio::TokioTestContext;
use rstest_bdd_macros::{given, scenario, scenarios, then, when};

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

#[then("the Tokio runtime remains available")]
fn tokio_runtime_remains_available() {
    let _handle = tokio::runtime::Handle::current();
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

/// Tests that an explicit default attribute policy can override harness-led
/// Tokio test attributes while the selected harness still provides a runtime.
#[scenario(
    path = "tests/features/tokio_harness.feature",
    name = "Tokio harness with default attribute override",
    harness = rstest_bdd_harness_tokio::TokioHarness,
    attributes = rstest_bdd_harness::DefaultAttributePolicy,
)]
fn scenario_runs_with_harness_and_default_policy_override() {}

/// Tests `#[scenario]` with `TokioAttributePolicy` and no harness.
#[scenario(
    path = "tests/features/tokio_harness.feature",
    name = "Tokio attribute policy without harness",
    attributes = rstest_bdd_harness_tokio::TokioAttributePolicy,
)]
async fn scenario_runs_with_attribute_policy_only() {}

// --- Async step definitions for TokioHarness ---

#[given("an async given step runs")]
async fn async_given_step(#[from(rstest_bdd_harness_context)] ctx: &TokioTestContext) {
    // Obtaining the handle proves the step is executing inside the harness runtime.
    let _handle = ctx.handle();
}

#[when("an async when step runs")]
async fn async_when_step(#[from(rstest_bdd_harness_context)] ctx: &TokioTestContext) {
    let _handle = ctx.handle();
}

#[then("the async steps completed")]
async fn async_steps_completed(#[from(rstest_bdd_harness_context)] ctx: &TokioTestContext) {
    // Spawning a local task confirms the LocalSet is active, not just the handle.
    tokio::task::spawn_local(async {});
    let _handle = ctx.handle();
}

/// Tests that `async fn` step definitions work with `TokioHarness`.
#[scenario(
    path = "tests/features/tokio_harness.feature",
    name = "Async step definitions execute under TokioHarness",
    harness = rstest_bdd_harness_tokio::TokioHarness,
)]
fn scenario_async_steps_under_tokio_harness() {}

scenarios!(
    "tests/features/tokio_harness_scenarios",
    harness = rstest_bdd_harness_tokio::TokioHarness,
);
