//! Trybuild fixture for `#[scenario]` step discovery when the step definition
//! macro is registered after the scenario binding.

use rstest_bdd_macros::{given, scenario};

#[scenario(path = "../features/macros/ambiguous.feature")]
fn out_of_order() {}

#[given("a step")]
fn a_step() {}

fn main() {}
