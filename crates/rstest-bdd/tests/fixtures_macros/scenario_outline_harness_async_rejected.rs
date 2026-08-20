//! Compile-fail fixture: `harness` combined with an async scenario outline is
//! rejected.
//!
//! The rejection must apply to the scenario-outline code path, not just to
//! regular scenarios, even though the outline is referenced through a passing
//! feature fixture with an Examples table.
use rstest_bdd_macros::{given, scenario, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(
    path = "../features/macros/outline.feature",
    harness = rstest_bdd_harness::StdHarness,
)]
async fn async_outline_with_harness(num: u32) {}

const _: &str = include_str!("../features/macros/outline.feature");

fn main() {}
