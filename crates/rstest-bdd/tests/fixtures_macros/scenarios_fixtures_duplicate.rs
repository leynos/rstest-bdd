//! Compile-fail fixture for `scenarios!`: verifies diagnostics for
//! duplicate `fixtures` argument.

use rstest_bdd_macros::scenarios;

scenarios!(
    "scenarios_fixtures_dir",
    fixtures = [a: A],
    fixtures = [b: B]
);

fn main() {}
