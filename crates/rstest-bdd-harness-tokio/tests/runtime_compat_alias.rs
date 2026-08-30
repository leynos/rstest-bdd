//! Behavioural tests for explicit Tokio harness selection in `scenarios!`.
//!
//! These tests verify that `rstest_bdd_harness_tokio::TokioHarness` provides a
//! Tokio current-thread runtime for synchronous scenario functions. Async step
//! definitions use explicit `async fn` scenarios or manual async tests instead.

use std::sync::atomic::{AtomicUsize, Ordering};

use rstest_bdd_macros::{given, scenarios, then, when};

/// Stores the synchronous counter exercised through the explicit Tokio harness.
static EXPLICIT_TOKIO_HARNESS_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Initializes the explicit Tokio-harness counter for the Given step.
#[given("an explicit Tokio harness counter initialized to 0")]
fn explicit_tokio_harness_counter_init() {
    EXPLICIT_TOKIO_HARNESS_COUNTER.store(0, Ordering::SeqCst);
}

/// Increments the explicit Tokio-harness counter for the When step.
#[when("the explicit Tokio harness counter is incremented synchronously")]
fn explicit_tokio_harness_counter_increment() {
    EXPLICIT_TOKIO_HARNESS_COUNTER.fetch_add(1, Ordering::SeqCst);
}

/// Verifies the explicit Tokio-harness counter value in the Then step.
#[then(expr = "the explicit Tokio harness counter value is {n}")]
fn explicit_tokio_harness_counter_value(n: usize) {
    let value = EXPLICIT_TOKIO_HARNESS_COUNTER.load(Ordering::SeqCst);
    assert_eq!(value, n, "expected counter to be {n}, got {value}");
}

scenarios!(
    "tests/features/runtime_compat_alias.feature",
    harness = rstest_bdd_harness_tokio::TokioHarness
);
