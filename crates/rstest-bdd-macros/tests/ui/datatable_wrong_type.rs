//! Compile-fail fixture: `#[datatable]` requires a type convertible from
//! `Vec<Vec<String>>`.

use rstest_bdd_macros::given;

type Wrong = String;

#[given("a step with wrong table type")]
fn step(#[datatable] _table: Wrong) {}

fn main() {}
