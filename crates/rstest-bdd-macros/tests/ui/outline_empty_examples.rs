//! Compile-fail test for empty Examples table validation.
use rstest_bdd_macros::scenario;

/// This test fails because the Examples block only contains headers.
#[scenario(path = "../../../../crates/rstest-bdd-macros/tests/features/outline_empty_examples.feature")]
fn compile_fail_empty_examples() {}

fn main() {}
