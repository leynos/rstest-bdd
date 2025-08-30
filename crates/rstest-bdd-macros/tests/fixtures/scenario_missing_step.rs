// compile-flags: --cfg feature="strict-compile-time-validation"
use rstest_bdd_macros::scenario;

#[scenario(path = "../../../../crates/rstest-bdd-macros/tests/features/unmatched.feature")]
fn missing_step() {}

fn main() {}
