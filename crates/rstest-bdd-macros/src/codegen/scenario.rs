//! Code generation for scenario tests.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};

/// Create a `LitStr` from an examples table cell.
fn cell_to_lit(value: &str) -> syn::LitStr {
    syn::LitStr::new(value, proc_macro2::Span::call_site())
}

/// Generate attributes for rstest cases based on examples.
pub(crate) fn generate_case_attrs(
    examples: &crate::parsing::examples::ExampleTable,
) -> Vec<TokenStream2> {
    examples
        .rows
        .iter()
        .filter(|row| row.iter().any(|cell| !cell.is_empty()))
        .map(|row| {
            let cells = row.iter().map(|v| {
                let lit = cell_to_lit(v);
                quote! { #lit }
            });
            quote! { #[case( #(#cells),* )] }
        })
        .collect()
}

/// Configuration for generating code for a single scenario test.
pub(crate) struct ScenarioConfig<'a> {
    /// Attributes on the annotated function.
    pub(crate) attrs: &'a [syn::Attribute],
    /// Visibility of the function.
    pub(crate) vis: &'a syn::Visibility,
    /// Signature of the function.
    pub(crate) sig: &'a syn::Signature,
    /// Function body.
    pub(crate) block: &'a syn::Block,
    /// Fully qualified feature file path.
    pub(crate) feature_path: String,
    /// Name of the scenario.
    pub(crate) scenario_name: String,
    /// Steps in the scenario.
    pub(crate) steps: Vec<crate::parsing::feature::ParsedStep>,
    /// Examples table for scenario outlines.
    pub(crate) examples: Option<crate::parsing::examples::ExampleTable>,
}

/// Generate tokens representing an optional data table.
///
/// # Examples
/// ```rust,ignore
/// let tokens = generate_table_tokens(None);
/// assert_eq!(tokens.to_string(), "None");
/// ```
fn generate_table_tokens(table: Option<&[Vec<String>]>) -> TokenStream2 {
    table.map_or_else(
        || quote! { None },
        |rows| {
            if rows.is_empty() {
                // Explicitly type the empty slice to avoid inference pitfalls when no rows exist.
                quote! { Some(&[] as &[&[&str]]) }
            } else {
                let row_tokens = rows.iter().map(|row| {
                    let cells = row.iter().map(|cell| {
                        let lit = cell_to_lit(cell);
                        quote! { #lit }
                    });
                    quote! { &[#(#cells),*][..] }
                });
                quote! { Some(&[#(#row_tokens),*][..]) }
            }
        },
    )
}

/// Process parsed steps into tokens for keywords, values, and tables.
///
/// # Examples
/// ```rust,ignore
/// use crate::StepKeyword;
/// use crate::parsing::feature::ParsedStep;
/// // Note: `span` is available only with the `compile-time-validation` feature.
/// let steps = vec![ParsedStep {
///     keyword: StepKeyword::Given,
///     text: "x".into(),
///     docstring: None,
///     table: None,
///     span: proc_macro2::Span::call_site(),
/// }];
/// let (k, v, t) = process_steps(&steps);
/// assert_eq!(v.len(), 1);
/// ```
fn process_steps(
    steps: &[crate::parsing::feature::ParsedStep],
) -> (
    Vec<TokenStream2>,
    Vec<TokenStream2>,
    Vec<TokenStream2>,
    Vec<TokenStream2>,
) {
    // Resolve textual conjunctions (And/But) to the previous primary keyword
    // without depending on the validation module, which is behind an optional
    // feature. We seed with the first primary keyword or Given by default.
    let keyword_tokens = {
        let mut prev = steps
            .iter()
            .find_map(|s| match s.keyword {
                crate::StepKeyword::And | crate::StepKeyword::But => None,
                other => Some(other),
            })
            .or(Some(crate::StepKeyword::Given));
        steps.iter().map(move |s| s.keyword.resolve(&mut prev))
    }
    .map(|kw| kw.to_token_stream())
    .collect::<Vec<_>>();
    debug_assert_eq!(keyword_tokens.len(), steps.len());
    let values = steps
        .iter()
        .map(|s| {
            let lit = cell_to_lit(&s.text);
            quote! { #lit }
        })
        .collect();
    let docstrings = steps
        .iter()
        .map(|s| {
            s.docstring.as_ref().map_or_else(
                || quote! { None },
                |d| {
                    let lit = syn::LitStr::new(d, proc_macro2::Span::call_site());
                    quote! { Some(#lit) }
                },
            )
        })
        .collect();
    let tables = steps
        .iter()
        .map(|s| generate_table_tokens(s.table.as_deref()))
        .collect();
    (keyword_tokens, values, docstrings, tables)
}

/// Grouped tokens for scenario steps.
struct ProcessedSteps {
    keyword_tokens: Vec<TokenStream2>,
    values: Vec<TokenStream2>,
    docstrings: Vec<TokenStream2>,
    tables: Vec<TokenStream2>,
}

/// Configuration for generating test tokens.
struct TestTokensConfig<'a> {
    processed_steps: ProcessedSteps,
    feature_path: &'a str,
    scenario_name: &'a str,
    block: &'a syn::Block,
}

/// Generate the inner body of the scenario test.
///
/// # Examples
/// ```rust,ignore
/// # use syn::parse_quote;
/// let processed = ProcessedSteps {
///     keyword_tokens: vec![],
///     values: vec![],
///     docstrings: vec![],
///     tables: vec![],
/// };
/// let config = TestTokensConfig {
///     processed_steps: processed,
///     feature_path: "feature",
///     scenario_name: "scenario",
///     block: &parse_quote!({}),
/// };
/// let body = generate_test_tokens(config, std::iter::empty());
/// assert!(body.to_string().contains("StepContext"));
/// ```
fn generate_test_tokens(
    config: TestTokensConfig<'_>,
    ctx_inserts: impl Iterator<Item = TokenStream2>,
) -> TokenStream2 {
    let TestTokensConfig {
        processed_steps:
            ProcessedSteps {
                keyword_tokens,
                values,
                docstrings,
                tables,
            },
        feature_path,
        scenario_name,
        block,
    } = config;

    let path = crate::codegen::rstest_bdd_path();
    quote! {
        let steps = [#((#keyword_tokens, #values, #docstrings, #tables)),*];
        let ctx = {
            let mut ctx = #path::StepContext::default();
            #(#ctx_inserts)*
            ctx
        };
        for (index, (keyword, text, docstring, table)) in steps.iter().enumerate() {
            if let Some(f) = #path::find_step(*keyword, (*text).into()) {
                if let Err(err) = f(&ctx, *text, *docstring, *table) {
                    panic!(
                        "Step failed at index {}: {} {} - {}\n(feature: {}, scenario: {})",
                        index,
                        keyword.as_str(),
                        text,
                        err,
                        #feature_path,
                        #scenario_name
                    );
                }
            } else {
                panic!(
                    "Step not found at index {}: {} {} (feature: {}, scenario: {})",
                    index,
                    keyword.as_str(),
                    text,
                    #feature_path,
                    #scenario_name
                );
            }
        }
        #block
    }
}

/// Generate the runtime test for a single scenario.
pub(crate) fn generate_scenario_code(
    config: ScenarioConfig<'_>,
    ctx_inserts: impl Iterator<Item = TokenStream2>,
) -> TokenStream {
    let ScenarioConfig {
        attrs,
        vis,
        sig,
        block,
        feature_path,
        scenario_name,
        steps,
        examples,
    } = config;
    let (keyword_tokens, values, docstrings, tables) = process_steps(&steps);
    debug_assert_eq!(keyword_tokens.len(), steps.len());
    let processed_steps = ProcessedSteps {
        keyword_tokens,
        values,
        docstrings,
        tables,
    };
    let test_config = TestTokensConfig {
        processed_steps,
        feature_path: &feature_path,
        scenario_name: &scenario_name,
        block,
    };
    let case_attrs = examples.map_or_else(Vec::new, |ex| generate_case_attrs(&ex));
    let body = generate_test_tokens(test_config, ctx_inserts);
    TokenStream::from(quote! {
        #[rstest::rstest]
        #(#case_attrs)*
        #(#attrs)*
        #vis #sig { #body }
    })
}

#[cfg(test)]
mod tests {
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
}
