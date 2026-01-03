//! Tests exercising scenario code-generation utilities.
use super::*;
use crate::parsing::feature::ParsedStep;

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

// -----------------------------------------------------------------------------
// Tests for generate_test_attrs: has_tokio_test detection and attribute generation
// -----------------------------------------------------------------------------

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
#[case::rstest_test("#[rstest::rstest]", false)]
#[case::tokio_runtime_not_test("#[tokio::main]", false)]
fn has_tokio_test_detection(#[case] attr_str: &str, #[case] expected_tokio: bool) {
    let attr = parse_attr(attr_str);
    let attrs = vec![attr];

    // When runtime is TokioCurrentThread and tokio::test is already present,
    // we should NOT emit another tokio::test attribute.
    let tokens = generate_test_attrs(&attrs, RuntimeMode::TokioCurrentThread);
    let has_tokio_in_output = tokens_contain(&tokens, "tokio :: test");

    if expected_tokio {
        // tokio::test detected, so output should NOT include tokio::test
        assert!(
            !has_tokio_in_output,
            "expected no tokio::test in output when user already has one: {attr_str}"
        );
    } else {
        // tokio::test NOT detected, so output SHOULD include tokio::test
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
    let tokens = generate_test_attrs(&attrs, runtime);
    let output = tokens.to_string();

    // All outputs should contain rstest::rstest
    assert!(
        output.contains("rstest :: rstest"),
        "expected rstest::rstest in output: {output}"
    );

    let has_tokio = output.contains("tokio :: test");
    assert_eq!(
        has_tokio, expect_tokio_test,
        "tokio::test presence mismatch for runtime={runtime:?}, attrs={attr_strs:?}"
    );

    // When tokio::test is emitted, it should specify current_thread flavor
    if expect_tokio_test {
        assert!(
            output.contains("current_thread"),
            "expected current_thread flavor in output: {output}"
        );
    }
}
