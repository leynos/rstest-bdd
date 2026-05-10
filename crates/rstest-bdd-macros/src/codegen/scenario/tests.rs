//! Tests exercising scenario code-generation utilities.
use super::test_attrs::{TestAttrPolicy, generate_test_attrs};
use super::*;
use crate::parsing::feature::ParsedStep;

mod gpui_policy;
mod harness_defaults;
mod runtime_split;
mod trait_assertions;

#[expect(clippy::expect_used, reason = "test helper with descriptive failures")]
fn kw(ts: &TokenStream2) -> crate::StepKeyword {
    let path = syn::parse2::<syn::Path>(ts.clone()).expect("keyword path");
    let ident = path.segments.last().expect("last").ident.to_string();
    crate::StepKeyword::try_from(ident.as_str()).expect("valid step keyword")
}

fn blank() -> ParsedStep {
    ParsedStep {
        keyword: crate::StepKeyword::Given,
        text: String::new(),
        docstring: None,
        table: None,
        #[cfg(feature = "compile-time-validation")]
        span: proc_macro2::Span::call_site(),
    }
}

#[rstest::rstest]
#[case::leading_and(
    vec![crate::StepKeyword::And, crate::StepKeyword::Then],
    vec![crate::StepKeyword::Then, crate::StepKeyword::Then],
)]
#[case::leading_but(
    vec![crate::StepKeyword::But, crate::StepKeyword::Then],
    vec![crate::StepKeyword::Then, crate::StepKeyword::Then],
)]
#[case::mixed(
    vec![crate::StepKeyword::Given, crate::StepKeyword::And, crate::StepKeyword::But, crate::StepKeyword::Then],
    vec![crate::StepKeyword::Given, crate::StepKeyword::Given, crate::StepKeyword::Given, crate::StepKeyword::Then],
)]
#[case::all_conjunctions(
    vec![crate::StepKeyword::And, crate::StepKeyword::But, crate::StepKeyword::And],
    vec![crate::StepKeyword::Given, crate::StepKeyword::Given, crate::StepKeyword::Given],
)]
#[case::empty(vec![], vec![])]
fn normalises_sequences(
    #[case] seq: Vec<crate::StepKeyword>,
    #[case] expect: Vec<crate::StepKeyword>,
) {
    let steps: Vec<_> = seq
        .into_iter()
        .map(|k| ParsedStep {
            keyword: k,
            ..blank()
        })
        .collect();
    let (keyword_tokens, _, _, _) = process_steps(&steps);
    let parsed: Vec<_> = keyword_tokens.iter().map(kw).collect();
    assert_eq!(parsed, expect);
}

fn tags(list: &[&str]) -> Vec<String> {
    list.iter().map(|tag| (*tag).to_owned()).collect()
}

#[rstest::rstest]
#[case::present(tags(&["@allow_skipped", "@other"]), true)]
#[case::absent(tags(&["@other", "@allow-skip"]), false)]
#[case::empty(Vec::<String>::new(), false)]
#[case::case_sensitive(tags(&["@Allow_Skipped"]), false)]
fn detects_allow_skipped_tag(#[case] tags: Vec<String>, #[case] expected: bool) {
    assert_eq!(scenario_allows_skip(&tags), expected);
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

fn tokens_contain(tokens: &TokenStream2, needle: &str) -> bool {
    tokens.to_string().contains(needle)
}

#[rstest::rstest]
#[case::tokio_test("#[tokio::test]", true)]
#[case::tokio_test_leading_colons("#[::tokio::test]", true)]
#[case::tokio_test_with_args("#[tokio::test(flavor = \"current_thread\")]", true)]
#[case::plain_test("#[test]", false)]
#[case::test_case("#[test_case]", false)]
#[case::tokio_test_case("#[tokio::test_case]", false)]
#[case::rstest_test("#[rstest::rstest]", false)]
#[case::tokio_runtime_not_test("#[tokio::main]", false)]
fn has_tokio_test_detection(#[case] attr_str: &str, #[case] expected_tokio: bool) {
    let attr = parse_attr(attr_str);
    let attrs = vec![attr];

    let tokens = generate_test_attrs(
        &attrs,
        &TestAttrPolicy {
            runtime: RuntimeMode::TokioCurrentThread,
            harness: None,
            attributes: None,
        },
        true,
    );
    let has_tokio_in_output = tokens_contain(&tokens, "tokio :: test");

    if expected_tokio {
        assert!(
            !has_tokio_in_output,
            "expected no tokio::test in output when user already has one: {attr_str}"
        );
    } else {
        assert!(
            has_tokio_in_output,
            "expected tokio::test in output when user does not have one: {attr_str}"
        );
    }
}

#[rstest::rstest]
#[case::sync_no_attrs(RuntimeMode::Sync, vec![], false)]
#[case::sync_with_tokio(RuntimeMode::Sync, vec!["#[tokio::test]"], false)]
#[case::tokio_no_attrs(RuntimeMode::TokioCurrentThread, vec![], true)]
#[case::tokio_with_tokio(RuntimeMode::TokioCurrentThread, vec!["#[tokio::test]"], false)]
#[case::tokio_with_test(RuntimeMode::TokioCurrentThread, vec!["#[test]"], true)]
fn generate_test_attrs_output(
    #[case] runtime: RuntimeMode,
    #[case] attr_strs: Vec<&str>,
    #[case] expect_tokio_test: bool,
) {
    let attrs: Vec<syn::Attribute> = attr_strs.iter().map(|s| parse_attr(s)).collect();
    let tokens = generate_test_attrs(
        &attrs,
        &TestAttrPolicy {
            runtime,
            harness: None,
            attributes: None,
        },
        runtime.is_async(),
    );
    let output = tokens.to_string();

    assert!(
        output.contains("rstest :: rstest"),
        "expected rstest::rstest in output: {output}"
    );

    let has_tokio = output.contains("tokio :: test");
    assert_eq!(
        has_tokio, expect_tokio_test,
        "tokio::test presence mismatch for runtime={runtime:?}, attrs={attr_strs:?}"
    );

    if expect_tokio_test {
        assert!(
            output.contains("current_thread"),
            "expected current_thread flavor in output: {output}"
        );
    }
}

#[expect(clippy::expect_used, reason = "test helper with descriptive failures")]
fn parse_path(s: &str) -> syn::Path {
    syn::parse_str::<syn::Path>(s).expect("valid path")
}

#[rstest::rstest]
#[case::with_default_policy_skips_tokio(
    Some(parse_path("rstest_bdd_harness::DefaultAttributePolicy")),
    false
)]
#[case::with_unknown_policy_skips_tokio(Some(parse_path("my::Policy")), false)]
#[case::with_tokio_policy_emits_tokio(
    Some(parse_path("rstest_bdd_harness_tokio::TokioAttributePolicy")),
    true
)]
#[case::with_absolute_tokio_policy_path_emits_tokio(
    Some(parse_path("::rstest_bdd_harness_tokio::TokioAttributePolicy")),
    true
)]
#[case::unresolved_tokio_policy(Some(parse_path("TokioAttributePolicy")), false)]
#[case::with_unknown_prefix_tokio_name_skips_tokio(
    Some(parse_path("my::TokioAttributePolicy")),
    false
)]
#[case::without_attributes_uses_runtime(None, true)]
fn generate_test_attrs_respects_attributes_policy(
    #[case] policy_path: Option<syn::Path>,
    #[case] expect_tokio_test: bool,
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
    let has_tokio = output.contains("tokio :: test");
    assert_eq!(
        has_tokio, expect_tokio_test,
        "tokio::test presence mismatch for policy={policy_path:?}: {output}"
    );
}

#[rstest::rstest]
#[case::tokio_policy_on_sync_function(Some(parse_path(
    "rstest_bdd_harness_tokio::TokioAttributePolicy"
)))]
#[case::runtime_tokio_on_sync_function(None)]
fn generate_test_attrs_omits_tokio_for_sync_functions(#[case] policy_path: Option<syn::Path>) {
    let policy = policy_path.as_ref();
    let tokens = generate_test_attrs(
        &[],
        &TestAttrPolicy {
            runtime: RuntimeMode::TokioCurrentThread,
            harness: None,
            attributes: policy,
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
        "should not contain tokio::test for sync functions: {output}"
    );
}

#[test]
fn generate_test_attrs_dedupes_tokio_policy_and_user_attribute() {
    let tokio_attr: syn::Attribute = syn::parse_quote!(#[tokio::test]);
    let attrs = vec![tokio_attr];

    let policy_path = parse_path("rstest_bdd_harness_tokio::TokioAttributePolicy");
    let generated_attrs = generate_test_attrs(
        &attrs,
        &TestAttrPolicy {
            runtime: RuntimeMode::TokioCurrentThread,
            harness: None,
            attributes: Some(&policy_path),
        },
        true,
    );

    // The final codegen emits existing function attributes plus generated
    // policy attributes. Verify that combination contains exactly one
    // tokio::test.
    let output = quote::quote! { #(#attrs)* #generated_attrs }.to_string();

    assert!(
        output.contains("rstest :: rstest"),
        "should contain rstest::rstest: {output}"
    );

    let tokio_count = output.match_indices("tokio :: test").count();
    assert_eq!(
        tokio_count, 1,
        "expected exactly one tokio::test when both user attribute and policy are present, got {tokio_count}: {output}"
    );
}
