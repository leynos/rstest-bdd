//! Compile-fail fixture: `#[datatable]` requires a type convertible from
//! `Vec<Vec<String>>`.

use rstest_bdd_macros::given;

// A local type with no `From`/`TryFrom` impls keeps the E0277 diagnostic free
// of rustc's candidate-impl suggestion list, whose rendering varies between
// stable releases.
struct Wrong;

#[given("a step with wrong table type")]
fn step(#[datatable] _table: Wrong) {}

fn main() {}
