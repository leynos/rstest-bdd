//! Compile-fail fixture for a malformed `scenarios!` library selection.

use rstest_bdd_macros::scenarios;

scenarios!("tests/features/auto", libraries = ["accounts"]);

fn main() {}
