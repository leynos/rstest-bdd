//! Unit tests for scenario test generation helpers.

use super::super::macro_args::FixtureSpec;
use super::super::macro_args::RuntimeCompatibilityAlias;
use super::{
    build_fixture_params, build_lint_attributes, build_test_signature, dedupe_name,
    resolve_harness_path,
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
        name: syn::parse_str(name).unwrap(),
        ty: syn::parse_str(ty).unwrap(),
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

#[test]
fn build_test_signature_no_fixtures_no_examples() {
    let fn_ident = syn::Ident::new("test_name", proc_macro2::Span::call_site());
    let sig = build_test_signature(&fn_ident, &[], &[], false);
    assert_eq!(sig_to_string(&sig), "fn test_name ()");
}

#[test]
fn build_test_signature_async_no_fixtures_no_examples() {
    let fn_ident = syn::Ident::new("test_name", proc_macro2::Span::call_site());
    let sig = build_test_signature(&fn_ident, &[], &[], true);
    assert_eq!(sig_to_string(&sig), "async fn test_name ()");
}

#[test]
fn build_test_signature_fixtures_only() {
    let fn_ident = syn::Ident::new("test_name", proc_macro2::Span::call_site());
    let fixture_params: Vec<TokenStream2> = vec![quote!(f1: T1), quote!(f2: T2)];

    let sig = build_test_signature(&fn_ident, &fixture_params, &[], false);
    let sig_str = sig_to_string(&sig);

    assert!(sig_str.contains("f1 : T1"), "should contain f1: T1");
    assert!(sig_str.contains("f2 : T2"), "should contain f2: T2");
}

#[test]
fn build_test_signature_async_fixtures_only() {
    let fn_ident = syn::Ident::new("test_name", proc_macro2::Span::call_site());
    let fixture_params: Vec<TokenStream2> = vec![quote!(f1: T1)];

    let sig = build_test_signature(&fn_ident, &fixture_params, &[], true);
    let sig_str = sig_to_string(&sig);

    assert!(sig_str.starts_with("async fn"), "should be async fn");
    assert!(sig_str.contains("f1 : T1"), "should contain f1: T1");
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
    let fn_ident = syn::Ident::new("test_name", proc_macro2::Span::call_site());
    let fixture_params: Vec<TokenStream2> = vec![quote!(world: TestWorld)];
    let example_params: Vec<TokenStream2> = vec![
        quote!(#[case] col1: &'static str),
        quote!(#[case] col2: &'static str),
    ];

    let sig = build_test_signature(&fn_ident, &fixture_params, &example_params, false);
    let sig_str = sig_to_string(&sig);

    let world_pos = sig_str.find("world").expect("should contain world");
    let col1_pos = sig_str.find("col1").expect("should contain col1");
    assert!(
        world_pos < col1_pos,
        "fixture 'world' should appear before example 'col1'"
    );
}

#[test]
fn build_test_signature_async_fixtures_then_examples() {
    let fn_ident = syn::Ident::new("test_name", proc_macro2::Span::call_site());
    let fixture_params: Vec<TokenStream2> = vec![quote!(world: TestWorld)];
    let example_params: Vec<TokenStream2> = vec![
        quote!(#[case] col1: &'static str),
        quote!(#[case] col2: &'static str),
    ];

    let sig = build_test_signature(&fn_ident, &fixture_params, &example_params, true);
    let sig_str = sig_to_string(&sig);

    assert!(sig_str.starts_with("async fn"), "should be async fn");

    let world_pos = sig_str.find("world").expect("should contain world");
    let col1_pos = sig_str.find("col1").expect("should contain col1");
    assert!(
        world_pos < col1_pos,
        "fixture 'world' should appear before example 'col1'"
    );
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
}

#[test]
fn resolve_harness_path_runtime_alias_does_not_force_harness_yet() {
    let resolved = resolve_harness_path(None, Some(RuntimeCompatibilityAlias::TokioHarnessAdapter));
    assert!(
        resolved.is_none(),
        "tokio compatibility alias keeps legacy runtime path until phase 9.3"
    );
}
