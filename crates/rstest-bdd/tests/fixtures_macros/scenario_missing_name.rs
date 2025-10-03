//! Compile-time fixture verifying that selecting a non-existent scenario by
//! name surfaces a descriptive error.
use rstest_bdd_macros::scenario;

#[scenario(path = "basic.feature", name = "Does not exist")]
fn missing_named_scenario() {}

const _: &str = include_str!("basic.feature");

fn main() {}
