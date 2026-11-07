//! Compile-fail fixture for a malformed `libraries` scenario argument.

use rstest_bdd_macros::scenario;

#[scenario(
    path = "step_libraries_ui.feature",
    libraries = ["accounts"],
)]
fn malformed_library_scenario() {}

fn main() {}
