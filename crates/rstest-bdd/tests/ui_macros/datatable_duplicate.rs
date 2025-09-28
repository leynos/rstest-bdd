//! Compile-fail fixture: multiple `#[datatable]` parameters are rejected.

use rstest_bdd_macros::given;

#[given("two datatables")]
fn step(#[datatable] first: Vec<Vec<String>>, #[datatable] second: Vec<Vec<String>>) {}

fn main() {}
