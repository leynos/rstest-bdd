//! Trybuild fixture for the `#[scenario]` macro warning emitted when a feature
//! file contains a step without a matching registered step definition.

use rstest_bdd_macros::scenario;

#[scenario(path = "../features/macros/unmatched.feature")]
fn missing_step() {}

// Force a compile error so trybuild captures the warning emitted by the macro.
compile_error!("forced failure");

fn main() {}
