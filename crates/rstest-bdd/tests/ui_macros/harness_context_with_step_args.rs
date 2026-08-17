//! Compile-fail fixture: `#[harness_context]` combined with `#[step_args]` is rejected.

use rstest_bdd_macros::given;

#[given("a step using conflicting attributes")]
fn step(
    #[harness_context]
    #[step_args]
    ctx: Args,
) {
}

fn main() {}
