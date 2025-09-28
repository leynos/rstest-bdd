use rstest_bdd_macros::scenario;

#[scenario(path = "../../../../crates/rstest-bdd/tests/features/macros/unmatched.feature")]
fn missing_step() {}

fn main() {}
