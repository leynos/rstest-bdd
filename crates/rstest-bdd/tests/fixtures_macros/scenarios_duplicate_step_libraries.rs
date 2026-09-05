//! Compile-fail fixture for a duplicate `scenarios!` library selection.

use rstest_bdd_macros::scenarios;

scenarios!("tests/features/auto", libraries = [accounts, accounts]);

fn main() {}
