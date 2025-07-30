use rstest_bdd_macros::scenario;

#[scenario(path = "../../../../crates/rstest-bdd-macros/tests/features/outline_missing_column.feature")]
fn compile_fail(num: String) {}

fn main() {}
