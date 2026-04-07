//! Compile-fail fixture verifying that Result-typed fixtures require a
//! Result-returning scenario.

use rstest_bdd_macros::scenario;

#[scenario(path = "basic.feature")]
fn result_fixture_unit_return(world: Result<u32, String>) {}

const _: &str = include_str!("basic.feature");

fn main() {}
