//! Compile-time fixture verifying that `name` and `index` cannot be combined.
use rstest_bdd_macros::scenario;

#[scenario(path = "basic.feature", name = "Example scenario", index = 0)]
fn scenario_with_name_and_index() {}

const _: &str = include_str!("basic.feature");

fn main() {}
