//! Compile-fail fixture: `#[harness_context]` combined with `#[from]` is rejected.

use rstest_bdd_macros::given;

#[given("a step using conflicting attributes")]
fn step(
    #[harness_context]
    #[from(rstest_bdd_harness_context)]
    ctx: &Context,
) {
}

fn main() {}
