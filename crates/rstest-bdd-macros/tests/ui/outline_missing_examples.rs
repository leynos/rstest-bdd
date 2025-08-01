//! Compile-fail test for Scenario Outline missing Examples table validation.
use rstest_bdd_macros::scenario;

/// This test ensures a compile error occurs when the Examples block is missing.
#[scenario(path = "../../../../crates/rstest-bdd-macros/tests/features/outline_missing_examples.feature")]
fn compile_fail_missing_examples() {}

fn main() {}
