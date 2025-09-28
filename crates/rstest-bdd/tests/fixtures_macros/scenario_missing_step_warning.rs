use rstest_bdd_macros::scenario;

#[scenario(path = "../features/macros/unmatched.feature")]
fn missing_step() {}

// Force a compile error so trybuild captures the warning emitted by the macro.
compile_error!("forced failure");

fn main() {}
