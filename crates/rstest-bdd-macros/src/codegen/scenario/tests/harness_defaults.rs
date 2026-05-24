//! Tests for harness-led default attribute-policy precedence.

use super::{RuntimeMode, TestAttrPolicy, generate_test_attrs};
use quote::quote;

#[expect(clippy::expect_used, reason = "test helper with descriptive failures")]
fn parse_path(s: &str) -> syn::Path {
    syn::parse_str::<syn::Path>(s).expect("valid path")
}

#[rstest::rstest]
// This table protects the ADR-008 precedence order:
//
// 1. explicit `attributes = ...` wins, even when it names another first-party
//    policy or an unknown policy;
// 2. fully qualified first-party harness paths imply their matching defaults;
// 3. unresolved or third-party-like harness paths fall back to the runtime; and
// 4. `attributes = ...` without `harness = ...` keeps the attributes-only
//    behaviour that existed before harness-led defaults.
#[case::tokio_harness_beats_sync_runtime(
    RuntimeMode::Sync,
    Some(parse_path("rstest_bdd_harness_tokio::TokioHarness")),
    None,
    true,
    false
)]
#[case::unresolved_tokio_harness_name_keeps_sync_runtime(
    RuntimeMode::Sync,
    Some(parse_path("TokioHarness")),
    None,
    false,
    false
)]
#[case::gpui_harness_beats_tokio_runtime(
    RuntimeMode::TokioCurrentThread,
    Some(parse_path("rstest_bdd_harness_gpui::GpuiHarness")),
    None,
    false,
    true
)]
#[case::unresolved_gpui_harness_name_keeps_tokio_runtime(
    RuntimeMode::TokioCurrentThread,
    Some(parse_path("GpuiHarness")),
    None,
    true,
    false
)]
#[case::std_harness_beats_tokio_runtime(
    RuntimeMode::TokioCurrentThread,
    Some(parse_path("rstest_bdd_harness::StdHarness")),
    None,
    false,
    false
)]
#[case::unknown_harness_falls_back_to_runtime(
    RuntimeMode::TokioCurrentThread,
    Some(parse_path("my::Harness")),
    None,
    true,
    false
)]
#[case::explicit_unknown_attributes_override_known_harness(
    RuntimeMode::TokioCurrentThread,
    Some(parse_path("rstest_bdd_harness_tokio::TokioHarness")),
    Some(parse_path("my::Policy")),
    false,
    false
)]
#[case::explicit_attributes_override_known_harness(
    RuntimeMode::Sync,
    Some(parse_path("rstest_bdd_harness_gpui::GpuiHarness")),
    Some(parse_path("rstest_bdd_harness_tokio::TokioAttributePolicy")),
    true,
    false
)]
#[case::explicit_gpui_attributes_override_tokio_harness(
    RuntimeMode::Sync,
    Some(parse_path("rstest_bdd_harness_tokio::TokioHarness")),
    Some(parse_path("rstest_bdd_harness_gpui::GpuiAttributePolicy")),
    false,
    true
)]
#[case::explicit_default_attributes_override_tokio_harness(
    RuntimeMode::TokioCurrentThread,
    Some(parse_path("rstest_bdd_harness_tokio::TokioHarness")),
    Some(parse_path("rstest_bdd_harness::DefaultAttributePolicy")),
    false,
    false
)]
#[case::tokio_attributes_only_use_tokio_policy(
    RuntimeMode::Sync,
    None,
    Some(parse_path("rstest_bdd_harness_tokio::TokioAttributePolicy")),
    true,
    false
)]
#[case::gpui_attributes_only_use_gpui_policy(
    RuntimeMode::TokioCurrentThread,
    None,
    Some(parse_path("rstest_bdd_harness_gpui::GpuiAttributePolicy")),
    false,
    true
)]
#[case::unknown_third_party_sync_harness_stays_rstest_only(
    RuntimeMode::Sync,
    Some(parse_path("third_party_harness::TokioHarness")),
    None,
    false,
    false
)]
#[case::unknown_third_party_like_gpui_harness_uses_runtime_fallback(
    RuntimeMode::TokioCurrentThread,
    Some(parse_path("third_party_harness::GpuiHarness")),
    None,
    true,
    false
)]
fn generate_test_attrs_honours_harness_precedence(
    #[case] runtime: RuntimeMode,
    #[case] harness_path: Option<syn::Path>,
    #[case] policy_path: Option<syn::Path>,
    #[case] expect_tokio_test: bool,
    #[case] expect_gpui_test: bool,
) {
    let harness = harness_path.as_ref();
    let policy = policy_path.as_ref();
    let tokens = generate_test_attrs(
        &[],
        &TestAttrPolicy {
            runtime,
            harness,
            attributes: policy,
        },
        true,
    );
    let output = tokens.to_string();

    assert!(
        output.contains("rstest :: rstest"),
        "should contain rstest::rstest: {output}"
    );
    assert_eq!(
        output.contains("tokio :: test"),
        expect_tokio_test,
        "tokio::test presence mismatch for runtime={runtime:?}, harness={harness_path:?}, policy={policy_path:?}: {output}"
    );
    assert_eq!(
        output.contains("gpui :: test"),
        expect_gpui_test,
        "gpui::test presence mismatch for runtime={runtime:?}, harness={harness_path:?}, policy={policy_path:?}: {output}"
    );
}

#[test]
fn tokio_harness_default_omits_tokio_for_sync_harness_function() {
    let harness_path = parse_path("rstest_bdd_harness_tokio::TokioHarness");
    let tokens = generate_test_attrs(
        &[],
        &TestAttrPolicy {
            runtime: RuntimeMode::Sync,
            harness: Some(&harness_path),
            attributes: None,
        },
        false,
    );
    let output = tokens.to_string();

    assert!(
        output.contains("rstest :: rstest"),
        "should contain rstest::rstest: {output}"
    );
    assert!(
        !output.contains("tokio :: test"),
        "Tokio harness defaults must not emit tokio::test for sync functions: {output}"
    );
}

#[test]
fn attributes_only_tokio_policy_emits_tokio_for_async_function() {
    // Attributes-only Tokio policy remains valid for generated async tests.
    let policy_path = parse_path("rstest_bdd_harness_tokio::TokioAttributePolicy");
    let tokens = generate_test_attrs(
        &[],
        &TestAttrPolicy {
            runtime: RuntimeMode::Sync,
            harness: None,
            attributes: Some(&policy_path),
        },
        true,
    );
    let output = tokens.to_string();

    assert!(
        output.contains("rstest :: rstest"),
        "should contain rstest::rstest: {output}"
    );
    assert!(
        output.contains("tokio :: test"),
        "Tokio attributes-only async functions should emit tokio::test: {output}"
    );
}

#[test]
fn attributes_only_tokio_policy_omits_tokio_for_sync_function() {
    // The same explicit policy must not produce an invalid `#[tokio::test]`
    // on generated synchronous test functions.
    let policy_path = parse_path("rstest_bdd_harness_tokio::TokioAttributePolicy");
    let tokens = generate_test_attrs(
        &[],
        &TestAttrPolicy {
            runtime: RuntimeMode::Sync,
            harness: None,
            attributes: Some(&policy_path),
        },
        false,
    );
    let output = tokens.to_string();

    assert!(
        output.contains("rstest :: rstest"),
        "should contain rstest::rstest: {output}"
    );
    assert!(
        !output.contains("tokio :: test"),
        "Tokio attributes-only sync functions must not emit tokio::test: {output}"
    );
}

#[rstest::rstest]
#[case::tokio(
    "#[tokio::test]",
    "rstest_bdd_harness_tokio::TokioHarness",
    "tokio :: test"
)]
#[case::gpui(
    "#[gpui::test]",
    "rstest_bdd_harness_gpui::GpuiHarness",
    "gpui :: test"
)]
fn generate_test_attrs_dedupes_harness_default_and_user_attribute(
    #[case] attr_str: &str,
    #[case] harness_path: &str,
    #[case] expected_attr: &str,
) {
    let user_attr = parse_attr(attr_str);
    let attrs = vec![user_attr];
    let harness_path = parse_path(harness_path);
    let generated_attrs = generate_test_attrs(
        &attrs,
        &TestAttrPolicy {
            runtime: RuntimeMode::TokioCurrentThread,
            harness: Some(&harness_path),
            attributes: None,
        },
        true,
    );
    let output = quote! { #(#attrs)* #generated_attrs }.to_string();

    assert!(
        output.contains("rstest :: rstest"),
        "should contain rstest::rstest: {output}"
    );
    assert_eq!(
        output.match_indices(expected_attr).count(),
        1,
        "expected exactly one {expected_attr} attribute: {output}"
    );
}

#[expect(clippy::expect_used, reason = "test helper with descriptive failures")]
fn parse_attr(s: &str) -> syn::Attribute {
    syn::parse_str::<syn::DeriveInput>(&format!("{s} struct S;"))
        .expect("parse derive input")
        .attrs
        .into_iter()
        .next()
        .expect("at least one attribute")
}
