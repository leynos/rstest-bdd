//! Behavioural tests for runtime compatibility alias resolution in `scenarios!`.
//!
//! These tests verify that `runtime = "tokio-current-thread"` remains usable
//! for async scenario execution while internally being treated as a compatibility
//! alias for Tokio harness selection.

use std::sync::atomic::{AtomicUsize, Ordering};

use rstest_bdd_macros::{given, scenarios, then, when};

static RUNTIME_ALIAS_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[given("a runtime alias counter initialised to 0")]
fn runtime_alias_counter_init() {
    RUNTIME_ALIAS_COUNTER.store(0, Ordering::SeqCst);
}

#[when("the runtime alias counter is incremented asynchronously")]
async fn runtime_alias_counter_increment() {
    RUNTIME_ALIAS_COUNTER.fetch_add(1, Ordering::SeqCst);
    tokio::task::yield_now().await;
}

#[then(expr = "the runtime alias counter value is {n}")]
fn runtime_alias_counter_value(n: usize) {
    let value = RUNTIME_ALIAS_COUNTER.load(Ordering::SeqCst);
    assert_eq!(value, n, "expected counter to be {n}, got {value}");
}

scenarios!(
    "tests/features/runtime_compat_alias.feature",
    runtime = "tokio-current-thread"
);
