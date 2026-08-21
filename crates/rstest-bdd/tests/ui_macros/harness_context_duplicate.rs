//! Compile-fail fixture: a duplicate `#[harness_context]` marker is rejected.

use rstest_bdd_macros::given;

#[given("a step using a duplicate marker")]
fn step(
    #[harness_context]
    #[harness_context]
    ctx: &Context,
) {
}

fn main() {}
