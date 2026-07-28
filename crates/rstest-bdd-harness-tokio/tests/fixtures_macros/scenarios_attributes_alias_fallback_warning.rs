//! Compile-fail fixture pinning the fallback diagnostic for an aliased Tokio
//! attribute policy used by `scenarios!`.
#![deny(deprecated)]

use rstest_bdd_macros::{given, scenarios, then, when};

mod alias {
    pub use rstest_bdd_harness_tokio;
}

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("an async result is produced")]
async fn async_result() {}

scenarios!(
    "scenarios_harness_tokio_default.feature",
    attributes = alias::rstest_bdd_harness_tokio::TokioAttributePolicy,
);

fn main() {}
