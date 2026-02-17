//! Compile-fail fixture: `harness` type implements `HarnessAdapter` but not `Default`.
use rstest_bdd_macros::{given, scenario, then, when};

struct NoDefaultHarness;

impl rstest_bdd_harness::HarnessAdapter for NoDefaultHarness {
    fn run<T>(&self, request: rstest_bdd_harness::ScenarioRunRequest<'_, T>) -> T {
        request.run()
    }
}

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(
    path = "basic.feature",
    harness = NoDefaultHarness,
)]
fn bad_default() {}

const _: &str = include_str!("basic.feature");

fn main() {}
