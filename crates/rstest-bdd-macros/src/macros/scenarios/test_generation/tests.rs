//! Unit tests for scenario test generation helpers.

use rstest::rstest;

use super::super::macro_args::FixtureSpec;
use super::super::macro_args::RuntimeCompatibilityAlias;
use super::super::macro_args::RuntimeMode;
use super::super::macro_args::runtime_compatibility_alias;
use super::{
    build_fixture_params, build_lint_attributes, build_test_signature, dedupe_name,
    resolve_effective_runtime, resolve_fixture_error_type, resolve_harness_path,
};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::collections::HashSet;

#[test]
fn deduplicates_duplicate_titles() {
    let mut used = HashSet::new();
    let first = dedupe_name("dup_same_name", &mut used);
    let second = dedupe_name("dup_same_name", &mut used);
    assert_eq!(first, "dup_same_name");
    assert_eq!(second, "dup_same_name_1");
}

fn make_fixture_spec(name: &str, ty: &str) -> FixtureSpec {
    FixtureSpec {
        name: syn::parse_str(name).expect("fixture name should parse"),
        ty: syn::parse_str(ty).expect("fixture type should parse"),
    }
}

fn sig_to_string(sig: &syn::Signature) -> String {
    quote!(#sig).to_string()
}

#[test]
fn build_lint_attributes_empty_fixtures_produces_no_attributes() {
    let attrs = build_lint_attributes(&[]);
    assert!(attrs.is_empty());
}

#[test]
fn build_lint_attributes_with_fixtures_produces_expect_attribute() {
    let fixtures = vec![make_fixture_spec("world", "TestWorld")];
    let attrs = build_lint_attributes(&fixtures);

    assert_eq!(attrs.len(), 1);
    let attr = &attrs[0];
    assert!(attr.path().is_ident("expect"));

    let attr_str = quote!(#attr).to_string();
    assert!(
        attr_str.contains("unused_variables"),
        "attribute should contain unused_variables: {attr_str}"
    );
    assert!(
        attr_str.contains("reason"),
        "attribute should contain reason: {attr_str}"
    );
    assert!(
        attr_str.contains("StepContext"),
        "reason should mention StepContext: {attr_str}"
    );
}

#[test]
fn build_lint_attributes_multiple_fixtures_still_produces_single_attribute() {
    let fixtures = vec![
        make_fixture_spec("world", "TestWorld"),
        make_fixture_spec("db", "Database"),
    ];
    let attrs = build_lint_attributes(&fixtures);
    assert_eq!(attrs.len(), 1);
}

#[rstest::rstest]
#[case::sync(false, "fn test_name ()")]
#[case::async_variant(true, "async fn test_name ()")]
fn build_test_signature_no_fixtures_no_examples(#[case] is_async: bool, #[case] expected: &str) {
    let fn_ident = syn::Ident::new("test_name", proc_macro2::Span::call_site());
    let sig = build_test_signature(&fn_ident, &[], &[], is_async);
    assert_eq!(sig_to_string(&sig), expected);
}

#[rstest::rstest]
#[case::sync(false, "fn")]
#[case::async_variant(true, "async fn")]
fn build_test_signature_fixtures_only(#[case] is_async: bool, #[case] prefix: &str) {
    let fn_ident = syn::Ident::new("test_name", proc_macro2::Span::call_site());
    let fixture_params: Vec<TokenStream2> = vec![quote!(f1: T1), quote!(f2: T2)];

    let sig = build_test_signature(&fn_ident, &fixture_params, &[], is_async);
    let sig_str = sig_to_string(&sig);

    assert!(sig_str.starts_with(prefix), "should start with {prefix}");
    assert!(sig_str.contains("f1 : T1"), "should contain f1: T1");
    assert!(sig_str.contains("f2 : T2"), "should contain f2: T2");
}

#[test]
fn build_test_signature_examples_only() {
    let fn_ident = syn::Ident::new("test_name", proc_macro2::Span::call_site());
    let example_params: Vec<TokenStream2> = vec![
        quote!(#[case] col1: &'static str),
        quote!(#[case] col2: &'static str),
    ];

    let sig = build_test_signature(&fn_ident, &[], &example_params, false);
    let sig_str = sig_to_string(&sig);

    assert!(sig_str.contains("# [case]"), "should contain #[case]");
    assert!(sig_str.contains("col1"), "should contain col1");
    assert!(sig_str.contains("col2"), "should contain col2");
}

#[test]
fn build_test_signature_fixtures_then_examples() {
    assert_fixtures_before_examples(false);
}

fn assert_fixtures_before_examples(is_async: bool) -> String {
    let fn_ident = syn::Ident::new("test_name", proc_macro2::Span::call_site());
    let fixture_params: Vec<TokenStream2> = vec![quote!(world: TestWorld)];
    let example_params: Vec<TokenStream2> = vec![
        quote!(#[case] col1: &'static str),
        quote!(#[case] col2: &'static str),
    ];

    let sig = build_test_signature(&fn_ident, &fixture_params, &example_params, is_async);
    let sig_str = sig_to_string(&sig);

    let world_pos = sig_str.find("world").expect("should contain world");
    let col1_pos = sig_str.find("col1").expect("should contain col1");
    assert!(
        world_pos < col1_pos,
        "fixture 'world' should appear before example 'col1'"
    );
    sig_str
}

#[test]
fn build_test_signature_async_fixtures_then_examples() {
    let sig_str = assert_fixtures_before_examples(true);
    assert!(sig_str.starts_with("async fn"), "should be async fn");
}

#[test]
fn build_fixture_params_empty() {
    let params = build_fixture_params(&[]);
    assert!(params.is_empty());
}

#[test]
fn build_fixture_params_single() {
    let fixtures = vec![make_fixture_spec("world", "TestWorld")];
    let params = build_fixture_params(&fixtures);

    assert_eq!(params.len(), 1);
    let param_str = params[0].to_string();
    assert!(param_str.contains("world"));
    assert!(param_str.contains("TestWorld"));
}

#[test]
fn build_fixture_params_multiple() {
    let fixtures = vec![
        make_fixture_spec("world", "TestWorld"),
        make_fixture_spec("db", "Database"),
    ];
    let params = build_fixture_params(&fixtures);

    assert_eq!(params.len(), 2);
}

#[test]
fn resolve_harness_path_prefers_explicit_harness() {
    let harness_path: syn::Path = syn::parse_str("my::Harness").expect("valid harness path");
    let resolved = resolve_harness_path(
        Some(&harness_path),
        Some(RuntimeCompatibilityAlias::TokioHarnessAdapter),
    );
    assert!(resolved.is_some(), "explicit harness should be preserved");
    let path_str = quote!(#resolved).to_string();
    assert!(path_str.contains("my") && path_str.contains("Harness"));
}

#[test]
fn resolve_harness_path_runtime_alias_resolves_to_tokio_harness() {
    let resolved = resolve_harness_path(None, Some(RuntimeCompatibilityAlias::TokioHarnessAdapter));
    assert!(
        resolved.is_some(),
        "tokio compatibility alias should resolve to TokioHarness path"
    );
    let path_str = quote!(#resolved).to_string();
    assert!(
        path_str.contains("rstest_bdd_harness_tokio") && path_str.contains("TokioHarness"),
        "resolved path should be rstest_bdd_harness_tokio::TokioHarness, got: {path_str}"
    );
}

// -- Tests for the effective_runtime / harness / signature pipeline ---
//
// These tests verify the combined behaviour of resolve_harness_path,
// resolve_effective_runtime, and build_test_signature, mirroring the
// pipeline inside generate_scenario_test without crossing the
// proc-macro API boundary.

#[rstest::rstest]
#[case::alias_without_explicit_harness(
    RuntimeMode::TokioCurrentThread,
    None,
    RuntimeMode::Sync,
    Some("rstest_bdd_harness_tokio"),
    Some("TokioHarness"),
    "fn "
)]
#[case::alias_with_explicit_harness(
    RuntimeMode::TokioCurrentThread,
    Some("my::ExplicitHarness"),
    RuntimeMode::TokioCurrentThread,
    Some("ExplicitHarness"),
    None,
    "async fn"
)]
#[case::sync_without_alias(RuntimeMode::Sync, None, RuntimeMode::Sync, None, None, "fn ")]
fn runtime_harness_signature_pipeline(
    #[case] runtime: RuntimeMode,
    #[case] explicit_harness_str: Option<&str>,
    #[case] expected_runtime: RuntimeMode,
    #[case] expected_harness_contains: Option<&str>,
    #[case] expected_harness_must_contain: Option<&str>,
    #[case] expected_sig_prefix: &str,
) {
    // Given: runtime and optional explicit harness
    let alias = runtime_compatibility_alias(runtime);
    let explicit_path: Option<syn::Path> =
        explicit_harness_str.map(|s| syn::parse_str(s).expect("valid path"));
    let explicit_harness = explicit_path.as_ref();

    // When: we resolve the harness and effective runtime
    let resolved_harness = resolve_harness_path(explicit_harness, alias);
    let effective_runtime = resolve_effective_runtime(runtime, alias, explicit_harness);

    // Then: effective runtime matches expected
    assert_eq!(effective_runtime, expected_runtime);

    // And: resolved harness matches expected presence/contents
    if let Some(must_contain) = expected_harness_must_contain {
        let harness = resolved_harness
            .as_ref()
            .expect("harness should be present");
        let harness_str = quote!(#harness).to_string();
        assert!(
            harness_str.contains(must_contain),
            "harness should contain {must_contain}, got: {harness_str}"
        );
    }
    if let Some(contains) = expected_harness_contains {
        if contains == "ExplicitHarness" {
            let harness = resolved_harness
                .as_ref()
                .expect("harness should be present");
            let harness_str = quote!(#harness).to_string();
            assert!(
                harness_str.contains(contains),
                "harness should contain {contains}, got: {harness_str}"
            );
            assert!(
                !harness_str.contains("rstest_bdd_harness_tokio"),
                "TokioHarness should not be injected with explicit harness"
            );
        } else if resolved_harness.is_none() {
            assert!(
                expected_harness_contains.is_none(),
                "expected no harness but got contains requirement"
            );
        } else {
            let harness = resolved_harness
                .as_ref()
                .expect("harness should be present");
            let harness_str = quote!(#harness).to_string();
            assert!(
                harness_str.contains(contains),
                "harness should contain {contains}, got: {harness_str}"
            );
        }
    } else {
        assert!(resolved_harness.is_none(), "expected no harness");
    }

    // And: the generated test signature starts with expected prefix
    let fn_ident = syn::Ident::new("test_scenario", proc_macro2::Span::call_site());
    let sig = build_test_signature(&fn_ident, &[], &[], effective_runtime.is_async());
    let sig_str = sig_to_string(&sig);
    assert!(
        sig_str.starts_with(expected_sig_prefix),
        "expected signature starting with {expected_sig_prefix}, got: {sig_str}"
    );
}

// -- Tests for resolve_fixture_error_type ---

#[rstest]
#[case("Result<MyWorld, String>")]
#[case("StepResult<MyWorld, String>")]
fn resolve_fixture_error_type_single_result_uses_fixture_error(#[case] fixture_ty: &str) {
    let fixtures = vec![make_fixture_spec("world", fixture_ty)];
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
        make_fixture_spec("world", fixture1_ty),
        make_fixture_spec("db", fixture2_ty),
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
        make_fixture_spec("world", "Result<MyWorld, String>"),
        make_fixture_spec("db", "Result<Database, std::io::Error>"),
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
    let fixtures = vec![make_fixture_spec("world", "MyWorld")];
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
        make_fixture_spec("plain", "MyWorld"),
        make_fixture_spec("fallible", fallible_ty),
    ];
    let error_ty = resolve_fixture_error_type(&fixtures);
    let error_str = quote!(#error_ty).to_string();
    assert!(
        error_str.contains("String"),
        "mixed fixtures with one result-like type should use its error type, got: {error_str}"
    );
}

#[test]
fn resolve_fixture_error_type_non_consecutive_duplicates_returns_shared_type() {
    // Tests that non-consecutive duplicate error types are deduplicated correctly
    // Pattern: Result<A, E>, Result<B, F>, Result<C, E>
    let fixtures = vec![
        make_fixture_spec("first", "Result<MyWorld, String>"),
        make_fixture_spec("second", "Result<Database, std::io::Error>"),
        make_fixture_spec("third", "Result<Config, String>"),
    ];
    let error_ty = resolve_fixture_error_type(&fixtures);
    let error_str = quote!(#error_ty).to_string();
    assert!(
        error_str.contains("Box"),
        "non-consecutive different error types should fall back to Box<dyn Error>, got: {error_str}"
    );
}

#[test]
fn resolve_fixture_error_type_all_same_non_consecutive_returns_shared_type() {
    // Tests that when all error types are the same but non-consecutive, we return the shared type
    // Pattern: Result<A, E>, Result<B, E>, Result<C, E>
    let fixtures = vec![
        make_fixture_spec("first", "Result<MyWorld, String>"),
        make_fixture_spec("second", "Result<Database, String>"),
        make_fixture_spec("third", "Result<Config, String>"),
    ];
    let error_ty = resolve_fixture_error_type(&fixtures);
    let error_str = quote!(#error_ty).to_string();
    assert!(
        error_str.contains("String"),
        "all same error types (even non-consecutive) should return shared type, got: {error_str}"
    );
}
