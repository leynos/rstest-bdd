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
