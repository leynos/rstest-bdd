//! Compile-pass fixture for `scenarios!` with an explicitly selected library.

use rstest_bdd_macros::{given, scenarios, step_library};

#[step_library]
mod accounts {
    //! Account-domain steps selected by the generated scenarios.

    use super::given;

    #[given("the account is empty")]
    fn account_is_empty() {}
}

scenarios!("scenarios_step_libraries_dir", libraries = [accounts]);

fn main() {}
