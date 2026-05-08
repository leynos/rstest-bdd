//! Trybuild fixture for the `#[scenario]` macro when the required feature-file
//! path argument is omitted.

use rstest_bdd_macros::scenario;

#[scenario(index = 0)]
fn missing_path_argument() {}

fn main() {}
