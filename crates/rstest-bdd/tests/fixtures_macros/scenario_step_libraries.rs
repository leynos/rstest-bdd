//! Compile-pass fixture for a scenario with an explicitly selected step library.

use rstest_bdd_macros::{given, scenario, step_library};

#[step_library]
mod accounts {
    //! Account-domain steps selected by this fixture's scenario.

    use super::given;

    #[given("the account is empty")]
    fn account_is_empty() {}
}

#[scenario(
    path = "step_libraries_ui.feature",
    libraries = [accounts],
)]
fn account_scenario() {}

const _: &str = include_str!("step_libraries_ui.feature");

fn main() {}
