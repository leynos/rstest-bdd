//! Tests for wrapper code generation helpers.

use super::{WrapperIdents, generate_wrapper_identifiers};
use crate::utils::ident::sanitize_ident;
use rstest::rstest;
use syn::parse_str;

#[rstest]
#[case(
    "préférence",
    3,
    "__rstest_bdd_wrapper_pr_f_rence_3",
    "__rstest_bdd_async_wrapper_pr_f_rence_3",
    "__RSTEST_BDD_FIXTURES_PR_F_RENCE_3",
    "__RSTEST_BDD_PATTERN_PR_F_RENCE_3"
)]
#[case(
    "数字",
    2,
    "__rstest_bdd_wrapper___2",
    "__rstest_bdd_async_wrapper___2",
    "__RSTEST_BDD_FIXTURES___2",
    "__RSTEST_BDD_PATTERN___2"
)]
#[case(
    "_1er_pas",
    4,
    "__rstest_bdd_wrapper__1er_pas_4",
    "__rstest_bdd_async_wrapper__1er_pas_4",
    "__RSTEST_BDD_FIXTURES__1ER_PAS_4",
    "__RSTEST_BDD_PATTERN__1ER_PAS_4"
)]
fn generates_ascii_only_idents(
    #[case] raw: &str,
    #[case] id: usize,
    #[case] expected_wrapper: &str,
    #[case] expected_async_wrapper: &str,
    #[case] expected_const: &str,
    #[case] expected_pattern: &str,
) {
    #[expect(clippy::expect_used, reason = "raw identifiers are test inputs")]
    let ident = parse_str::<syn::Ident>(raw).expect("parse identifier");
    let WrapperIdents {
        sync_wrapper,
        async_wrapper,
        const_ident,
        pattern_ident,
    } = generate_wrapper_identifiers(&ident, id);

    // Verify wrapper ident derives from the sanitized base.
    let base = sanitize_ident(&ident.to_string());
    assert!(
        sync_wrapper.to_string().ends_with(&format!("{base}_{id}")),
        "wrapper ident must include sanitized base and id",
    );

    // Exact expectations
    assert_eq!(sync_wrapper.to_string(), expected_wrapper);
    assert_eq!(async_wrapper.to_string(), expected_async_wrapper);
    assert_eq!(const_ident.to_string(), expected_const);
    assert_eq!(pattern_ident.to_string(), expected_pattern);

    // ASCII-only invariants
    assert!(sync_wrapper.to_string().is_ascii());
    assert!(async_wrapper.to_string().is_ascii());
    assert!(const_ident.to_string().is_ascii());
    assert!(pattern_ident.to_string().is_ascii());
}
