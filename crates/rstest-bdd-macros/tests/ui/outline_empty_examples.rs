// The Examples block in the feature contains only headers and should cause
// a compile-time error.
use rstest_bdd_macros::scenario;

#[scenario(path = "../../../../crates/rstest-bdd-macros/tests/features/outline_empty_examples.feature")]
fn compile_fail() {}

fn main() {}
