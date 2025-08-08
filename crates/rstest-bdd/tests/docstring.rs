//! Behavioural test for doc string support

use rstest_bdd_macros::{given, scenario};

#[given("the following message:")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "doc string is owned to mirror user API"
)]
fn check_docstring(docstring: String) {
    assert_eq!(docstring, "\nhello\nworld\n");
}

#[scenario(path = "tests/features/docstring.feature")]
fn docstring_scenario() {}
