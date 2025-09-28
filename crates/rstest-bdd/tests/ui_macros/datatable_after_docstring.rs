//! Compile-fail fixture: `#[datatable]` must precede a doc string parameter.

use rstest_bdd_macros::given;

#[given("datatable after docstring")]
fn step(docstring: String, #[datatable] table: Vec<Vec<String>>) {}

fn main() {}
