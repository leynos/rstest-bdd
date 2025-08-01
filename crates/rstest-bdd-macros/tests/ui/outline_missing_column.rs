// The Examples table in the feature has a missing column value and should cause
// a compile-time error.
use rstest_bdd_macros::scenario;

#[scenario(path = "../../../../crates/rstest-bdd-macros/tests/features/outline_missing_column.feature")]
fn compile_fail_missing_column(num: String) {}

fn main() {}
