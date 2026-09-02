//! Compile-fail fixture: `#[harness_context]` on a placeholder-bound parameter.

use rstest_bdd_macros::given;

#[given("a step {count}")]
fn step(
    #[harness_context]
    count: &Context,
) {
}

fn main() {}
