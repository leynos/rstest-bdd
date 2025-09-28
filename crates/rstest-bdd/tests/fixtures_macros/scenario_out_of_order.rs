use rstest_bdd_macros::{given, scenario};

#[scenario(path = "../../../../crates/rstest-bdd/tests/features/macros/ambiguous.feature")]
fn out_of_order() {}

#[given("a step")]
fn a_step() {}

fn main() {}
