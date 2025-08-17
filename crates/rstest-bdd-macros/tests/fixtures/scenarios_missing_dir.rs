//! Compile-fail fixture for `scenarios!`: verifies diagnostics for a missing
//! features directory.

use rstest_bdd_macros::scenarios;

scenarios!("tests/features/does_not_exist");

fn main() {}
