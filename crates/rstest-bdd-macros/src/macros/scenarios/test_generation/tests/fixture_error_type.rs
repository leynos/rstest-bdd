//! Unit tests for fixture error-type resolution in generated scenarios.

use super::*;

// -- Tests for resolve_fixture_error_type ---

#[rstest]
#[case("Result<MyWorld, String>")]
#[case("StepResult<MyWorld, String>")]
fn resolve_fixture_error_type_single_result_uses_fixture_error(#[case] fixture_ty: &str) {
    let fixtures = vec![make_fixture_spec!("world", fixture_ty)];
    let error_ty = resolve_fixture_error_type(&fixtures);
    let error_str = quote!(#error_ty).to_string();
    assert!(
        error_str.contains("String"),
        "single result-like fixture should use its error type, got: {error_str}"
    );
    assert!(
        !error_str.contains("Box"),
        "single result-like fixture should not use Box<dyn Error>, got: {error_str}"
    );
}

#[rstest]
#[case("Result<MyWorld, String>", "Result<Database, String>")]
#[case("StepResult<MyWorld, String>", "StepResult<Database, String>")]
#[case("Result<MyWorld, String>", "StepResult<Database, String>")]
fn resolve_fixture_error_type_multiple_same_error_uses_shared_type(
    #[case] fixture1_ty: &str,
    #[case] fixture2_ty: &str,
) {
    let fixtures = vec![
        make_fixture_spec!("world", fixture1_ty),
        make_fixture_spec!("db", fixture2_ty),
    ];
    let error_ty = resolve_fixture_error_type(&fixtures);
    let error_str = quote!(#error_ty).to_string();
    assert!(
        error_str.contains("String"),
        "fixtures sharing the same error type should use it directly, got: {error_str}"
    );
}

#[test]
fn resolve_fixture_error_type_different_errors_falls_back_to_box() {
    let fixtures = vec![
        make_fixture_spec!("world", "Result<MyWorld, String>"),
        make_fixture_spec!("db", "Result<Database, std::io::Error>"),
    ];
    let error_ty = resolve_fixture_error_type(&fixtures);
    let error_str = quote!(#error_ty).to_string();
    assert!(
        error_str.contains("Box"),
        "different error types should fall back to Box<dyn Error>, got: {error_str}"
    );
}

#[test]
fn resolve_fixture_error_type_no_result_fixtures_falls_back_to_box() {
    let fixtures = vec![make_fixture_spec!("world", "MyWorld")];
    let error_ty = resolve_fixture_error_type(&fixtures);
    let error_str = quote!(#error_ty).to_string();
    assert!(
        error_str.contains("Box"),
        "no Result fixtures should fall back to Box<dyn Error>, got: {error_str}"
    );
}

#[rstest]
#[case("Result<Database, String>")]
#[case("StepResult<Database, String>")]
fn resolve_fixture_error_type_mixed_plain_and_result_uses_result_error(#[case] fallible_ty: &str) {
    let fixtures = vec![
        make_fixture_spec!("plain", "MyWorld"),
        make_fixture_spec!("fallible", fallible_ty),
    ];
    let error_ty = resolve_fixture_error_type(&fixtures);
    let error_str = quote!(#error_ty).to_string();
    assert!(
        error_str.contains("String"),
        "mixed fixtures with one result-like type should use its error type, got: {error_str}"
    );
}

#[rstest]
#[case(
    "Result<Database, std::io::Error>",
    "Box",
    "non-consecutive different error types should fall back to Box<dyn Error>"
)]
#[case(
    "Result<Database, String>",
    "String",
    "all same error types (even non-consecutive) should return the shared type"
)]
fn resolve_fixture_error_type_non_consecutive_three_fixtures(
    #[case] second_ty: &str,
    #[case] expected_fragment: &str,
    #[case] msg: &str,
) {
    let fixtures = vec![
        make_fixture_spec!("first", "Result<MyWorld, String>"),
        make_fixture_spec!("second", second_ty),
        make_fixture_spec!("third", "Result<Config, String>"),
    ];
    let error_ty = resolve_fixture_error_type(&fixtures);
    let error_str = quote!(#error_ty).to_string();
    assert!(
        error_str.contains(expected_fragment),
        "{msg}, got: {error_str}"
    );
}
