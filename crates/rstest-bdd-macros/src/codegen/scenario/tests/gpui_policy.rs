//! GPUI-specific attribute policy tests for scenario test-attribute generation.

use super::{RuntimeMode, generate_test_attrs, parse_path};

#[rstest::rstest]
#[case::with_gpui_policy_emits_gpui(Some(parse_path(
    "rstest_bdd_harness_gpui::GpuiAttributePolicy"
)))]
#[case::with_absolute_gpui_policy_path_emits_gpui(Some(parse_path(
    "::rstest_bdd_harness_gpui::GpuiAttributePolicy"
)))]
fn generate_test_attrs_respects_gpui_policy_paths(#[case] policy_path: Option<syn::Path>) {
    let policy = policy_path.as_ref();
    let tokens = generate_test_attrs(&[], RuntimeMode::TokioCurrentThread, None, policy, true);
    let output = tokens.to_string();

    assert!(
        output.contains("rstest :: rstest"),
        "should contain rstest::rstest: {output}"
    );
    assert!(
        output.contains("gpui :: test"),
        "should contain gpui::test when GPUI policy is selected: {output}"
    );
    assert!(
        !output.contains("tokio :: test"),
        "should not contain tokio::test when GPUI policy is selected: {output}"
    );
}

#[test]
fn generate_test_attrs_emits_gpui_for_sync_functions() {
    let policy_path = parse_path("rstest_bdd_harness_gpui::GpuiAttributePolicy");
    let tokens = generate_test_attrs(&[], RuntimeMode::Sync, None, Some(&policy_path), false);
    let output = tokens.to_string();

    assert!(
        output.contains("rstest :: rstest"),
        "should contain rstest::rstest: {output}"
    );
    assert!(
        output.contains("gpui :: test"),
        "should contain gpui::test for sync functions: {output}"
    );
}

#[test]
fn generate_test_attrs_dedupes_gpui_policy_and_user_attribute() {
    let gpui_attr: syn::Attribute = syn::parse_quote!(#[gpui::test]);
    let attrs = vec![gpui_attr];

    let policy_path = parse_path("rstest_bdd_harness_gpui::GpuiAttributePolicy");
    let generated_attrs =
        generate_test_attrs(&attrs, RuntimeMode::Sync, None, Some(&policy_path), false);
    let output = quote::quote! { #(#attrs)* #generated_attrs }.to_string();

    assert!(
        output.contains("rstest :: rstest"),
        "should contain rstest::rstest: {output}"
    );

    let gpui_count = output.match_indices("gpui :: test").count();
    assert_eq!(
        gpui_count, 1,
        "expected exactly one gpui::test when both user attribute and policy are present, got {gpui_count}: {output}"
    );
}
