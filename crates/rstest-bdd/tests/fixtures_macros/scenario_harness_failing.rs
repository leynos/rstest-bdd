//! Compile-pass fixture validating generated delegation for harness failures.
use rstest_bdd_harness::{
    HarnessAdapter, HarnessError, HarnessResult, ScenarioRunRequest,
};
use rstest_bdd_macros::{given, scenario, then, when};
use std::io;

#[derive(Default)]
struct FailingHarness;

impl HarnessAdapter for FailingHarness {
    type Context = ();

    fn run<T>(&self, _request: ScenarioRunRequest<'_, Self::Context, T>) -> HarnessResult<T> {
        Err(HarnessError::RuntimeBuildFailed(io::Error::other(
            "synthetic harness failure",
        )))
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
    harness = FailingHarness,
)]
fn failing_harness_delegation_compiles() {}

const _: &str = include_str!("basic.feature");

fn main() {}
