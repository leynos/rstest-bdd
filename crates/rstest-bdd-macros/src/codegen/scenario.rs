//! Code generation for scenario tests.

use super::keyword_to_token;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

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
                return quote! { Some(&[] as &[&[&str]]) };
            }
            let row_tokens = rows.iter().map(|row| {
                let cells = row.iter().map(|cell| {
                    let lit = cell_to_lit(cell);
                    quote! { #lit }
                });
                quote! { &[#(#cells),*][..] }
            });
            quote! { Some(&[#(#row_tokens),*][..]) }
        },
    )
}

/// Process parsed steps into tokens for keywords, values, and tables.
///
/// # Examples
/// ```rust,ignore
/// use rstest_bdd::StepKeyword;
/// use crate::parsing::feature::ParsedStep;
/// let steps = vec![ParsedStep { keyword: StepKeyword::Given, text: "x".into(), table: None }];
/// let (k, v, t) = process_steps(&steps);
/// assert_eq!(v.len(), 1);
/// ```
fn process_steps(
    steps: &[crate::parsing::feature::ParsedStep],
) -> (Vec<TokenStream2>, Vec<TokenStream2>, Vec<TokenStream2>) {
    let keywords = steps.iter().map(|s| keyword_to_token(s.keyword)).collect();
    let values = steps
        .iter()
        .map(|s| {
            let lit = syn::LitStr::new(&s.text, proc_macro2::Span::call_site());
            quote! { #lit }
        })
        .collect();
    let tables = steps
        .iter()
        .map(|s| generate_table_tokens(s.table.as_deref()))
        .collect();
    (keywords, values, tables)
}

/// Grouped tokens for scenario steps.
struct ProcessedSteps {
    keywords: Vec<TokenStream2>,
    values: Vec<TokenStream2>,
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
///     keywords: vec![],
///     values: vec![],
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
                keywords,
                values,
                tables,
            },
        feature_path,
        scenario_name,
        block,
    } = config;

    quote! {
        let steps = [#((#keywords, #values, #tables)),*];
        let mut ctx = rstest_bdd::StepContext::default();
        #(#ctx_inserts)*
        for (index, (keyword, text, table)) in steps.iter().enumerate() {
            if let Some(f) = rstest_bdd::find_step(*keyword, (*text).into()) {
                if let Err(err) = f(&ctx, *text, *table) {
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
    let (keywords, values, tables) = process_steps(&steps);
    let processed_steps = ProcessedSteps {
        keywords,
        values,
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
