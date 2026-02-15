//! Compile-fail fixture: `harness` combined with `async fn` is rejected.
use rstest_bdd_macros::{given, scenario, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(
    path = "basic.feature",
    harness = rstest_bdd_harness::StdHarness,
)]
async fn async_with_harness() {}

const _: &str = include_str!("basic.feature");

fn main() {}
