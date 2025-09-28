//! Compile-fail fixture: duplicate `#[datatable]` attribute on a single parameter is rejected.

use rstest_bdd_macros::given;

#[given("duplicate attribute")]
fn step(#[datatable] #[datatable] table: Vec<Vec<String>>) {}

fn main() {}
