//! GPUI-specific attribute policy tests for scenario test-attribute generation.

use super::{
    RuntimeMode, ScenarioReturnKind, TestAttrPolicy, adapt_fallible_gpui_boundary,
    generate_test_attrs,
};
use crate::codegen::scenario::test_attrs::generate_test_attrs_with_boundary;

#[rstest::rstest]
#[case::with_gpui_policy_emits_gpui(
    Some(parse_path!("rstest_bdd_harness_gpui::GpuiAttributePolicy")),
    true
)]
#[case::with_absolute_gpui_policy_path_emits_gpui(
    Some(parse_path!("::rstest_bdd_harness_gpui::GpuiAttributePolicy")),
    true
)]
#[case::with_unresolved_gpui_policy_name_skips_gpui(Some(parse_path!("GpuiAttributePolicy")), false)]
fn generate_test_attrs_respects_gpui_policy_paths(
    #[case] policy_path: Option<syn::Path>,
    #[case] expect_gpui_test: bool,
) {
    let policy = policy_path.as_ref();
    let tokens = generate_test_attrs(
        &[],
        &TestAttrPolicy {
            runtime: RuntimeMode::TokioCurrentThread,
            harness: None,
            attributes: policy,
        },
        true,
    );
    let output = tokens.to_string();

    assert!(
        output.contains("rstest :: rstest"),
        "should contain rstest::rstest: {output}"
    );
    assert!(
        output.contains("gpui :: test") == expect_gpui_test,
        "gpui::test presence mismatch for policy={policy_path:?}: {output}"
    );
    assert!(
        !output.contains("tokio :: test"),
        "should not contain tokio::test when GPUI policy is selected: {output}"
    );
}

#[test]
fn generate_test_attrs_emits_gpui_for_sync_functions() {
    let policy_path = parse_path!("rstest_bdd_harness_gpui::GpuiAttributePolicy");
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
        output.contains("gpui :: test"),
        "should contain gpui::test for sync functions: {output}"
    );
}

#[test]
fn generate_test_attrs_dedupes_gpui_policy_and_user_attribute() {
    let gpui_attr: syn::Attribute = syn::parse_quote!(#[gpui::test]);
    let attrs = vec![gpui_attr];

    let policy_path = parse_path!("rstest_bdd_harness_gpui::GpuiAttributePolicy");
    let generated_attrs = generate_test_attrs(
        &attrs,
        &TestAttrPolicy {
            runtime: RuntimeMode::Sync,
            harness: None,
            attributes: Some(&policy_path),
        },
        false,
    );
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

fn adapt_boundary(
    attrs: &[syn::Attribute],
    policy: &TestAttrPolicy<'_>,
    return_kind: ScenarioReturnKind,
    is_async: bool,
) -> (syn::Signature, String) {
    let is_fallible = return_kind.is_fallible();
    let mut signature: syn::Signature = if is_fallible {
        syn::parse_quote!(fn generated_scenario() -> Result<(), String>)
    } else {
        syn::parse_quote!(fn generated_scenario())
    };
    let body = if is_fallible {
        quote::quote! { Ok::<(), String>(()) }
    } else {
        quote::quote! { () }
    };
    if is_async {
        signature.asyncness = Some(syn::parse_quote!(async));
    }
    let generated_test_attrs = generate_test_attrs_with_boundary(attrs, policy, is_async);
    let body = adapt_fallible_gpui_boundary(
        generated_test_attrs.uses_gpui_boundary,
        return_kind,
        &mut signature,
        body,
    );
    (signature, body.to_string())
}

#[rstest::rstest]
#[case::explicit_policy(
    None,
    Some(parse_path!("rstest_bdd_harness_gpui::GpuiAttributePolicy"))
)]
#[case::inferred_harness(
    Some(parse_path!("rstest_bdd_harness_gpui::GpuiHarness")),
    None
)]
fn fallible_gpui_boundaries_consume_the_scenario_result(
    #[case] harness: Option<syn::Path>,
    #[case] attributes: Option<syn::Path>,
) {
    let policy = TestAttrPolicy {
        runtime: RuntimeMode::Sync,
        harness: harness.as_ref(),
        attributes: attributes.as_ref(),
    };
    let (signature, body) = adapt_boundary(&[], &policy, ScenarioReturnKind::ResultUnit, false);

    assert!(matches!(signature.output, syn::ReturnType::Default));
    assert!(
        body.contains("match (||"),
        "expected a sync result boundary: {body}"
    );
    assert!(
        body.contains("scenario returned an error"),
        "expected a diagnostic for a scenario error: {body}"
    );
    assert!(
        !body.contains("__rstest_bdd_err"),
        "the GPUI boundary must not inspect the error value: {body}"
    );
}

#[test]
fn direct_gpui_attribute_consumes_the_scenario_result() {
    let attrs = vec![syn::parse_quote!(#[gpui::test])];
    let policy = TestAttrPolicy {
        runtime: RuntimeMode::Sync,
        harness: None,
        attributes: None,
    };
    let (signature, _) = adapt_boundary(&attrs, &policy, ScenarioReturnKind::ResultUnit, false);

    assert!(matches!(signature.output, syn::ReturnType::Default));
}

#[test]
fn async_gpui_boundary_awaits_and_consumes_the_scenario_result() {
    let attributes = parse_path!("rstest_bdd_harness_gpui::GpuiAttributePolicy");
    let policy = TestAttrPolicy {
        runtime: RuntimeMode::TokioCurrentThread,
        harness: None,
        attributes: Some(&attributes),
    };
    let (signature, body) = adapt_boundary(&[], &policy, ScenarioReturnKind::ResultUnit, true);

    assert!(matches!(signature.output, syn::ReturnType::Default));
    assert!(
        body.contains("async move") && body.contains(". await"),
        "expected an awaited async result boundary: {body}"
    );
    assert!(
        body.contains("scenario returned an error") && !body.contains("__rstest_bdd_err"),
        "the async GPUI boundary must use the fixed panic message: {body}"
    );
}

#[rstest::rstest]
#[case::std(RuntimeMode::Sync, None)]
#[case::tokio(
    RuntimeMode::TokioCurrentThread,
    Some(parse_path!("rstest_bdd_harness_tokio::TokioAttributePolicy"))
)]
fn non_gpui_boundaries_preserve_fallible_return_signatures(
    #[case] runtime: RuntimeMode,
    #[case] attributes: Option<syn::Path>,
) {
    let policy = TestAttrPolicy {
        runtime,
        harness: None,
        attributes: attributes.as_ref(),
    };
    let (signature, body) = adapt_boundary(
        &[],
        &policy,
        ScenarioReturnKind::ResultUnit,
        runtime.is_async(),
    );

    assert!(matches!(signature.output, syn::ReturnType::Type(..)));
    assert_eq!(body, "Ok :: < () , String > (())");
}

#[test]
fn unit_gpui_scenarios_keep_their_body_unchanged() {
    let attributes = parse_path!("rstest_bdd_harness_gpui::GpuiAttributePolicy");
    let policy = TestAttrPolicy {
        runtime: RuntimeMode::Sync,
        harness: None,
        attributes: Some(&attributes),
    };
    let (signature, body) = adapt_boundary(&[], &policy, ScenarioReturnKind::Unit, false);

    assert!(matches!(signature.output, syn::ReturnType::Default));
    assert_eq!(body, "()");
}
