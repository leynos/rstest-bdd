use rstest_bdd_macros::scenario;

#[scenario(path = "../../../../crates/rstest-bdd-macros/tests/features/outline_empty_examples.feature")]
fn compile_fail() {}

fn main() {}
