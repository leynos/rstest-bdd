//! Compile-fail test for duplicate headers in Examples table.
use rstest_bdd_macros::scenario;

/// This test ensures duplicate headers trigger a compile-time error.
#[scenario(path = "../../../../crates/rstest-bdd-macros/tests/features/outline_duplicate_headers.feature")]
fn compile_fail_duplicate_headers() {}

fn main() {}
