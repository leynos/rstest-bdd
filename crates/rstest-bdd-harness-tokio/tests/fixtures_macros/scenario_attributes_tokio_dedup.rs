//! Compile-pass fixture validating that `#[scenario]` does not emit duplicate
//! Tokio test attributes when one is already present on the test function.
use rstest_bdd_macros::{given, scenario, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(
    path = "basic.feature",
    attributes = rstest_bdd_harness_tokio::TokioAttributePolicy,
)]
#[tokio::test(flavor = "current_thread")]
async fn with_tokio_attributes_policy_dedup() {}

// Compile-time guard: fail fast if the feature path changes.
const _: &str = include_str!("basic.feature");

fn main() {}
