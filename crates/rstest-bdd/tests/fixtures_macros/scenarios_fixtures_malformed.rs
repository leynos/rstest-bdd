//! Compile-fail fixture for `scenarios!`: verifies diagnostics for
//! malformed fixture entries (missing type/colon).

use rstest_bdd_macros::scenarios;

scenarios!(
    "scenarios_fixtures_dir",
    fixtures = [world]
);

fn main() {}
