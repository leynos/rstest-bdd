//! Compile-fail fixture: `#[harness_context(...)]` carries arguments.

use rstest_bdd_macros::given;

#[given("a step using an argued marker")]
fn step(
    #[harness_context(gpui)]
    ctx: &Context,
) {
}

fn main() {}
