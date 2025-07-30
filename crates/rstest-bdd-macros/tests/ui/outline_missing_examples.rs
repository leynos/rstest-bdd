use rstest_bdd_macros::scenario;

#[scenario(path = "../../../../crates/rstest-bdd-macros/tests/features/outline_missing_examples.feature")]
fn compile_fail() {}

fn main() {}
