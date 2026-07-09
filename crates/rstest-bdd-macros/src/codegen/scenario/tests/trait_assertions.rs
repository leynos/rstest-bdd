//! Tests for generated compile-time trait assertions.

use super::generate_trait_assertions;

#[derive(Clone, Copy)]
enum ParamKind {
    Harness,
    Attributes,
}

#[derive(Clone, Copy, Debug)]
enum TraitName {
    HarnessAdapter,
    AttributePolicy,
}

impl TraitName {
    fn as_str(self) -> &'static str {
        match self {
            Self::HarnessAdapter => "HarnessAdapter",
            Self::AttributePolicy => "AttributePolicy",
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum ExpectedCrate {
    TokioAdapter,
    GpuiAdapter,
    BaseHarness,
}

impl ExpectedCrate {
    fn as_str(self) -> &'static str {
        match self {
            Self::TokioAdapter => "rstest_bdd_harness_tokio",
            Self::GpuiAdapter => "rstest_bdd_harness_gpui",
            Self::BaseHarness => "rstest_bdd_harness",
        }
    }
}

fn assert_single_trait_assertion(
    param_kind: ParamKind,
    path: &syn::Path,
    expected_trait: TraitName,
    excluded_trait: TraitName,
) {
    let (harness, attributes) = match param_kind {
        ParamKind::Harness => (Some(path), None),
        ParamKind::Attributes => (None, Some(path)),
    };
    let tokens = generate_trait_assertions(harness, attributes);
    let output = tokens.to_string();
    let expected = expected_trait.as_str();
    let excluded = excluded_trait.as_str();

    assert!(
        output.contains(expected),
        "should contain {expected} trait bound: {output}"
    );
    let spaced_path = quote::quote!(#path).to_string();
    assert!(
        output.contains(&spaced_path),
        "should contain type path `{spaced_path}`: {output}"
    );
    assert!(
        !output.contains(excluded),
        "should NOT contain {excluded}: {output}"
    );
}

fn assert_trait_assertion_crate_path(
    param_kind: ParamKind,
    path: &syn::Path,
    expected_trait: TraitName,
    expected_crate: ExpectedCrate,
) {
    let (harness, attributes) = match param_kind {
        ParamKind::Harness => (Some(path), None),
        ParamKind::Attributes => (None, Some(path)),
    };
    let tokens = generate_trait_assertions(harness, attributes);
    let output = tokens.to_string();
    let expected_t = expected_trait.as_str();
    let expected_c = expected_crate.as_str();

    assert!(
        output.contains(expected_t),
        "should contain trait `{expected_t}`: {output}"
    );
    assert!(
        output.contains(expected_c),
        "should resolve through `{expected_c}`: {output}"
    );
}

#[rstest::rstest]
#[case::tokio_harness_uses_tokio_crate(
    ParamKind::Harness,
    parse_path!("rstest_bdd_harness_tokio::TokioHarness"),
    TraitName::HarnessAdapter,
    ExpectedCrate::TokioAdapter
)]
#[case::tokio_policy_uses_tokio_crate(
    ParamKind::Attributes,
    parse_path!("rstest_bdd_harness_tokio::TokioAttributePolicy"),
    TraitName::AttributePolicy,
    ExpectedCrate::TokioAdapter
)]
#[case::gpui_policy_uses_gpui_crate(
    ParamKind::Attributes,
    parse_path!("rstest_bdd_harness_gpui::GpuiAttributePolicy"),
    TraitName::AttributePolicy,
    ExpectedCrate::GpuiAdapter
)]
#[case::custom_harness_uses_base_harness_crate(
    ParamKind::Harness,
    parse_path!("my_crate::MyHarness"),
    TraitName::HarnessAdapter,
    ExpectedCrate::BaseHarness
)]
#[case::custom_policy_uses_base_harness_crate(
    ParamKind::Attributes,
    parse_path!("my_crate::MyPolicy"),
    TraitName::AttributePolicy,
    ExpectedCrate::BaseHarness
)]
fn trait_assertions_resolve_correct_crate_path(
    #[case] kind: ParamKind,
    #[case] path: syn::Path,
    #[case] expected_trait: TraitName,
    #[case] expected_crate: ExpectedCrate,
) {
    assert_trait_assertion_crate_path(kind, &path, expected_trait, expected_crate);
}

#[rstest::rstest]
#[case::harness(
    ParamKind::Harness,
    parse_path!("my::Harness"),
    TraitName::HarnessAdapter,
    TraitName::AttributePolicy
)]
#[case::attributes(
    ParamKind::Attributes,
    parse_path!("my::Policy"),
    TraitName::AttributePolicy,
    TraitName::HarnessAdapter
)]
fn trait_assertions_single_param(
    #[case] kind: ParamKind,
    #[case] path: syn::Path,
    #[case] expected_trait: TraitName,
    #[case] excluded_trait: TraitName,
) {
    assert_single_trait_assertion(kind, &path, expected_trait, excluded_trait);
}

#[test]
fn trait_assertions_with_both() {
    let harness_path = parse_path!("my::Harness");
    let policy_path = parse_path!("my::Policy");
    let tokens = generate_trait_assertions(Some(&harness_path), Some(&policy_path));
    let output = tokens.to_string();

    assert!(
        output.contains("HarnessAdapter"),
        "should contain HarnessAdapter: {output}"
    );
    assert!(
        output.contains("AttributePolicy"),
        "should contain AttributePolicy: {output}"
    );
}

#[test]
fn trait_assertions_with_neither() {
    let tokens = generate_trait_assertions(None, None);
    let output = tokens.to_string();

    assert!(
        !output.contains("HarnessAdapter"),
        "should NOT contain HarnessAdapter: {output}"
    );
    assert!(
        !output.contains("AttributePolicy"),
        "should NOT contain AttributePolicy: {output}"
    );
}

#[test]
fn trait_assertions_harness_includes_default_bound() {
    let harness_path = parse_path!("my::Harness");
    let tokens = generate_trait_assertions(Some(&harness_path), None);
    let output = tokens.to_string();

    assert!(
        output.contains("HarnessAdapter"),
        "should contain HarnessAdapter: {output}"
    );
    assert!(
        output.contains("Default"),
        "should contain Default bound for harness delegation: {output}"
    );
}
