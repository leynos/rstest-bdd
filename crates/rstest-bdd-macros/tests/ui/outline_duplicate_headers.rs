// This test ensures duplicate headers in the Examples table produce an error.
use rstest_bdd_macros::scenario;

#[scenario(path = "../../../../crates/rstest-bdd-macros/tests/features/outline_duplicate_headers.feature")]
fn compile_fail_duplicate_headers() {}

fn main() {}
