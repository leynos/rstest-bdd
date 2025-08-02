//! Compile-fail test for missing column values in Examples table.
use rstest_bdd_macros::scenario;

/// This test fails when a row has fewer columns than the header.
#[scenario(path = "../../../../crates/rstest-bdd-macros/tests/features/outline_missing_column.feature")]
fn compile_fail_missing_column(num: String) {}

fn main() {}
