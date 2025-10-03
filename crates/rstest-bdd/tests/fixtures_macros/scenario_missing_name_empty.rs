//! Compile-time fixture verifying that selecting a scenario from an empty
//! feature file produces a clear diagnostic.
use rstest_bdd_macros::scenario;

#[scenario(path = "empty.feature", name = "Any scenario")]
fn missing_scenario_in_empty_feature() {}

const _: &str = include_str!("empty.feature");

fn main() {}
