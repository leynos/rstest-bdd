//! Compile-pass fixture for `scenarios!`: verifies autodiscovery succeeds when
//! the feature directory exists.

use rstest_bdd_macros::scenarios;

scenarios!("tests/features/auto");

fn main() {}
