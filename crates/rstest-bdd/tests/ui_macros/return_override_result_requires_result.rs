//! UI compile-fail fixture ensuring `result` override requires `Result`.

use rstest_bdd_macros::when;

#[when(result)]
fn not_a_result() -> u8 {
    1
}

fn main() {}
