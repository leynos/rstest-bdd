use rstest_bdd_macros::{given, scenario};

#[given("a step")]
fn first() {}

#[given("a step")]
fn second() {}

#[scenario(path = "../../../../crates/rstest-bdd-macros/tests/features/ambiguous.feature")]
fn ambiguous() {}

fn main() {}
