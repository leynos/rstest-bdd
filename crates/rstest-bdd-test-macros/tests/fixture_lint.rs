//! Proves fixture lint allowances remain scoped to macro-generated code.

#![deny(unused_braces)]

use rstest::{fixture, rstest};

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn expression_fixture() -> u8 { 7 }

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn multi_statement_fixture() -> u8 {
    let mut fixture_value = 10;
    fixture_value += 1;
    fixture_value
}

#[rstest]
fn fixture_lint_allowance_supports_each_fixture_shape(
    expression_fixture: u8,
    multi_statement_fixture: u8,
) {
    assert_eq!(expression_fixture + multi_statement_fixture, 18);
}
