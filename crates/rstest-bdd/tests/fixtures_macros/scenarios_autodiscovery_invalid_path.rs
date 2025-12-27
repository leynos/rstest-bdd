//! Compile-fail fixture for `scenarios!`: verifies diagnostics for an
//! invalid autodiscovery path.

use rstest_bdd_macros::scenarios;

scenarios!("tests/features/nonexistent_auto");

fn main() {}
