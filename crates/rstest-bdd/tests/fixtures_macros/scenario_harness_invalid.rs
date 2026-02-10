//! Compile-fail fixture: `harness` type does not implement `HarnessAdapter`.
use rstest_bdd_macros::{given, scenario, then, when};

struct NotAHarness;

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(
    path = "basic.feature",
    harness = NotAHarness,
)]
fn bad_harness() {}

const _: &str = include_str!("basic.feature");

fn main() {}
