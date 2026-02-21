//! Integration tests verifying that `#[scenario]` works with the Tokio harness
//! adapter from `rstest-bdd-harness-tokio`.

use rstest_bdd_macros::{given, scenario, then, when};
use std::sync::atomic::{AtomicBool, Ordering};

static RUNTIME_ACTIVE: AtomicBool = AtomicBool::new(false);
static HANDLE_OBTAINED: AtomicBool = AtomicBool::new(false);

#[given("the Tokio runtime is active")]
fn tokio_runtime_is_active() {
    // Panics if no Tokio runtime is active on the current thread.
    let _handle = tokio::runtime::Handle::current();
    RUNTIME_ACTIVE.store(true, Ordering::SeqCst);
}

#[when("a Tokio handle is obtained")]
fn tokio_handle_is_obtained() {
    let _handle = tokio::runtime::Handle::current();
    HANDLE_OBTAINED.store(true, Ordering::SeqCst);
}

#[then("the handle confirms current-thread execution")]
fn handle_confirms_current_thread() {
    assert!(
        RUNTIME_ACTIVE.load(Ordering::SeqCst),
        "Tokio runtime should have been active in the Given step"
    );
    assert!(
        HANDLE_OBTAINED.load(Ordering::SeqCst),
        "Tokio handle should have been obtained in the When step"
    );
}

#[scenario(
    path = "tests/features/tokio_harness.feature",
    harness = rstest_bdd_harness_tokio::TokioHarness,
)]
fn scenario_runs_inside_tokio_runtime() {
    // Reset flags for next test run.
    RUNTIME_ACTIVE.store(false, Ordering::SeqCst);
    HANDLE_OBTAINED.store(false, Ordering::SeqCst);
}
