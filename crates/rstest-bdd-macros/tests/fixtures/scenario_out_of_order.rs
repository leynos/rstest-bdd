use rstest_bdd_macros::{given, scenario};

#[scenario(path = "../../../../crates/rstest-bdd-macros/tests/features/ambiguous.feature")]
fn out_of_order() {}

#[given("a step")]
fn a_step() {}

fn main() {}
