//! Compile-pass fixture combining a local library with a cross-crate library.

use rstest_bdd_macros::{given, scenario, step_library};

#[step_library]
mod accounts {
    //! Account-domain steps visible to compile-time validation.

    use super::given;

    #[given("a local account step")]
    fn local_account_step() {}
}

#[scenario(
    path = "local_and_cross_crate_libraries.feature",
    libraries = [accounts, rstest_bdd::global],
)]
fn mixed_library_scenario() {}

const _: &str = include_str!("local_and_cross_crate_libraries.feature");

fn main() {}
