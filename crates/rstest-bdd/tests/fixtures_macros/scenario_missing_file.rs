//! Trybuild fixture for the `#[scenario]` macro when the requested feature
//! file does not exist.

use rstest_bdd_macros::scenario;

#[scenario(path = "tests/features/does_not_exist.feature")]
fn missing() {}

fn main() {}
