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
    /// Whether the scenario permits skipping without failing the suite.
    pub(crate) allow_skipped: bool,
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

pub(crate) fn scenario_allows_skip(tags: &[String]) -> bool {
    tags.iter().any(|tag| tag == "@allow_skipped")
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
    allow_skipped: bool,
}

fn execute_single_step(_feature_path: &str, _scenario_name: &str) -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
    quote! {
        #[allow(
            clippy::too_many_arguments,
            reason = "helper mirrors generated step inputs to keep panic messaging intact",
        )]
        fn execute_single_step(
            index: usize,
            keyword: #path::StepKeyword,
            text: &str,
            docstring: Option<&str>,
            table: Option<&[&[&str]]>,
            ctx: &#path::StepContext,
            feature_path: &str,
            scenario_name: &str,
        ) -> Result<Option<Box<dyn std::any::Any>>, String> {
            if let Some(f) = #path::find_step(keyword, text.into()) {
                match f(ctx, text, docstring, table) {
                    Ok(#path::StepExecution::Continue { value }) => Ok(value),
                    Ok(#path::StepExecution::Skipped { message }) => {
                        Err(message.unwrap_or_default())
                    }
                    Err(err) => {
                        panic!(
                            "Step failed at index {}: {} {} - {}\n(feature: {}, scenario: {})",
                            index,
                            keyword.as_str(),
                            text,
                            err,
                            feature_path,
                            scenario_name
                        );
                    }
                }
            } else {
                panic!(
                    "Step not found at index {}: {} {} (feature: {}, scenario: {})",
                    index,
                    keyword.as_str(),
                    text,
                    feature_path,
                    scenario_name
                );
            }
        }
    }
}

fn validate_skip_result(_feature_path: &str, _scenario_name: &str) -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
    quote! {
        fn validate_skip_result(
            skipped: Option<String>,
            allow_skipped: bool,
            feature_path: &str,
            scenario_name: &str,
        ) -> bool {
            if let Some(message) = skipped {
                if #path::config::fail_on_skipped() && !allow_skipped {
                    let detail = if message.is_empty() {
                        "scenario skipped"
                    } else {
                        message.as_str()
                    };
                    panic!(
                        "Scenario skipped with fail_on_skipped enabled: {}\n(feature: {}, scenario: {})",
                        detail,
                        feature_path,
                        scenario_name
                    );
                }
                false
            } else {
                true
            }
        }
    }
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
        allow_skipped,
    } = config;

    let path = crate::codegen::rstest_bdd_path();
    let allow_literal = syn::LitBool::new(allow_skipped, proc_macro2::Span::call_site());
    let feature_literal = syn::LitStr::new(feature_path, proc_macro2::Span::call_site());
    let scenario_literal = syn::LitStr::new(scenario_name, proc_macro2::Span::call_site());
    let step_executor = execute_single_step(feature_path, scenario_name);
    let skip_validator = validate_skip_result(feature_path, scenario_name);
    quote! {
        const FEATURE_PATH: &str = #feature_literal;
        const SCENARIO_NAME: &str = #scenario_literal;
        #step_executor
        #skip_validator

        let steps = [#((#keyword_tokens, #values, #docstrings, #tables)),*];
        let allow_skipped: bool = #allow_literal;
        let mut ctx = {
            let mut ctx = #path::StepContext::default();
            #(#ctx_inserts)*
            ctx
        };
        let mut skipped: Option<String> = None;
        for (index, (keyword, text, docstring, table)) in steps.iter().enumerate() {
            match execute_single_step(
                index,
                *keyword,
                *text,
                *docstring,
                *table,
                &ctx,
                FEATURE_PATH,
                SCENARIO_NAME,
            ) {
                Ok(value) => {
                    if let Some(val) = value {
                        let _ = ctx.insert_value(val);
                    }
                }
                Err(encoded) => {
                    skipped = Some(encoded);
                    break;
                }
            }
        }
        if !validate_skip_result(skipped, allow_skipped, FEATURE_PATH, SCENARIO_NAME) {
            return;
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
        allow_skipped,
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
        allow_skipped,
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
mod tests;
