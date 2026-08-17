//! Compile-fail fixture: `#[harness_context]` combined with `#[datatable]` is rejected.

use rstest_bdd_macros::given;

#[given("a step using conflicting attributes")]
fn step(
    #[harness_context]
    #[datatable]
    ctx: &Context,
) {
}

fn main() {}
