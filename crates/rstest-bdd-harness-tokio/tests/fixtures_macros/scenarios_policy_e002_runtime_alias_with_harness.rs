//! Compile-fail fixture pinning E002 for the deprecated alias combined with an
//! explicit harness.
//!
//! The feature declares two scenarios, so this also pins that the replacement
//! error is reported once for the invocation rather than once per generated
//! test.
use rstest_bdd_macros::{given, scenarios, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("an async result is produced")]
async fn async_result() {}

scenarios!(
    "scenarios_multi_fallback.feature",
    runtime = "tokio-current-thread",
    harness = rstest_bdd_harness_tokio::TokioHarness,
);

fn main() {}
