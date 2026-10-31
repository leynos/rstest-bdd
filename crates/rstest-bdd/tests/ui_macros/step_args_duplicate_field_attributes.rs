//! UI compile-fail fixture for invalid `StepArgs` field configuration.

use rstest_bdd_macros::StepArgs;

#[derive(StepArgs)]
struct InvalidArgs {
    #[step_args(placeholder = "first", placeholder = "second")]
    value: String,
}

fn main() {}
