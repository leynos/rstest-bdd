//! Compile-fail fixture for `scenarios!`: a directory whose features declare
//! no scenarios is an error naming the path, not an empty module.

use rstest_bdd_macros::scenarios;

scenarios!("scenarios_empty_dir");

fn main() {}
