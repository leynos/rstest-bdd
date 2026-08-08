//! Compile-pass fixture for a fallible GPUI outline with a non-`Debug` error.

#![deny(warnings)]

use rstest_bdd_macros::{given, scenario, then, when};

struct NonDebugError;

#[given("a precondition for {case}")]
fn precondition(case: String) {
    assert_eq!(case, "one");
}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(
    path = "fallible_outline.feature",
    harness = rstest_bdd_harness_gpui::GpuiHarness,
)]
fn fallible_outline_scenario(_case: String) -> Result<(), NonDebugError> { Ok(()) }

const _: &str = include_str!("fallible_outline.feature");

fn main() {}
