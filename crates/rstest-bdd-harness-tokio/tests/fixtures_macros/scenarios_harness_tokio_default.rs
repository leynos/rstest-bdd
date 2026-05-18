//! Compile-pass fixture validating that `scenarios!` accepts a first-party
//! Tokio harness without an explicit attribute policy.
use rstest_bdd_macros::{given, scenarios, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("an async result is produced")]
async fn async_result() {}

scenarios!(
    "scenarios_harness_tokio_default.feature",
    harness = rstest_bdd_harness_tokio::TokioHarness,
);

fn main() {}
