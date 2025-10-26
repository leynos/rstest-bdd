//! Code generation for scenario tests.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

mod helpers;
mod metadata;

use helpers::generate_case_attrs;
pub(crate) use helpers::process_steps;
pub(crate) use metadata::{FeaturePath, ScenarioName};

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
    pub(crate) feature_path: FeaturePath,
    /// Name of the scenario.
    pub(crate) scenario_name: ScenarioName,
    /// Steps in the scenario.
    pub(crate) steps: Vec<crate::parsing::feature::ParsedStep>,
    /// Examples table for scenario outlines.
    pub(crate) examples: Option<crate::parsing::examples::ExampleTable>,
    /// Whether the scenario permits skipping without failing the suite.
    pub(crate) allow_skipped: bool,
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
    feature_path: &'a FeaturePath,
    scenario_name: &'a ScenarioName,
    block: &'a syn::Block,
    allow_skipped: bool,
}

fn execute_single_step(feature_path: &FeaturePath, scenario_name: &ScenarioName) -> TokenStream2 {
    let _ = feature_path.as_str();
    let _ = scenario_name.as_str();
    let path = crate::codegen::rstest_bdd_path();
    quote! {
        #[expect(
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
                        let encoded = message.map_or_else(
                            || SKIP_NONE_PREFIX.to_string(),
                            |msg| {
                                let mut encoded = String::with_capacity(1 + msg.len());
                                encoded.push(SKIP_SOME_PREFIX);
                                encoded.push_str(&msg);
                                encoded
                            },
                        );
                        Err(encoded)
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

fn validate_skip_result(feature_path: &FeaturePath, scenario_name: &ScenarioName) -> TokenStream2 {
    let _ = feature_path.as_str();
    let _ = scenario_name.as_str();
    let path = crate::codegen::rstest_bdd_path();
    quote! {
        fn validate_skip_result(
            skipped: Option<Option<String>>,
            allow_skipped: bool,
            feature_path: &str,
            scenario_name: &str,
        ) -> bool {
            if let Some(message) = skipped {
                if #path::config::fail_on_skipped() && !allow_skipped {
                    let detail = message.unwrap_or_else(|| "scenario skipped".to_string());
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

fn generate_skip_decoder() -> TokenStream2 {
    quote! {
        fn decode_skip_message(encoded: String) -> Option<String> {
            match encoded.chars().next() {
                Some(prefix) if prefix == SKIP_NONE_PREFIX => None,
                Some(prefix) if prefix == SKIP_SOME_PREFIX => {
                    let prefix_len = prefix.len_utf8();
                    Some(encoded[prefix_len..].to_string())
                }
                _ => Some(encoded),
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
///     feature_path: &FeaturePath::new("feature"),
///     scenario_name: &ScenarioName::new("scenario"),
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
    let feature_literal = syn::LitStr::new(feature_path.as_str(), proc_macro2::Span::call_site());
    let scenario_literal = syn::LitStr::new(scenario_name.as_str(), proc_macro2::Span::call_site());
    let step_executor = execute_single_step(feature_path, scenario_name);
    let skip_validator = validate_skip_result(feature_path, scenario_name);
    let skip_decoder = generate_skip_decoder();
    quote! {
        const FEATURE_PATH: &str = #feature_literal;
        const SCENARIO_NAME: &str = #scenario_literal;
        const SKIP_NONE_PREFIX: char = '\u{0}';
        const SKIP_SOME_PREFIX: char = '\u{1}';
        #step_executor
        #skip_validator
        #skip_decoder

        let steps = [#((#keyword_tokens, #values, #docstrings, #tables)),*];
        let allow_skipped: bool = #allow_literal;
        let mut ctx = {
            let mut ctx = #path::StepContext::default();
            #(#ctx_inserts)*
            ctx
        };
        let mut skipped: Option<Option<String>> = None;
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
                    skipped = Some(decode_skip_message(encoded));
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
