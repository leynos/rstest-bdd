//! Compile-pass fixture validating that `scenarios!` still accepts the
//! canonical Tokio attribute-policy path without a harness.
use rstest_bdd_macros::{given, scenarios, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("an async result is produced")]
async fn async_result() {}

scenarios!(
    "scenarios_harness_tokio_default.feature",
    attributes = rstest_bdd_harness_tokio::TokioAttributePolicy,
);

fn main() {}
