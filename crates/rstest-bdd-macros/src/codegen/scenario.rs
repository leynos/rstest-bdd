//! Code generation for scenario tests.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

mod helpers;
mod metadata;
mod runtime;

use helpers::generate_case_attrs;
pub(crate) use helpers::process_steps;
pub(crate) use metadata::{FeaturePath, ScenarioName};
use runtime::{generate_test_tokens, ProcessedSteps, TestTokensConfig};

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
    /// Line number where the scenario is declared in the feature file.
    pub(crate) line: u32,
    /// Tags inherited from the feature and scenario declarations.
    pub(crate) tags: &'a [String],
}

pub(crate) fn scenario_allows_skip(tags: &[String]) -> bool {
    tags.iter().any(|tag| tag == "@allow_skipped")
}

/// Generate the runtime test for a single scenario.
pub(crate) fn generate_scenario_code(
    config: ScenarioConfig<'_>,
    ctx_prelude: impl Iterator<Item = TokenStream2>,
    ctx_inserts: impl Iterator<Item = TokenStream2>,
    ctx_postlude: impl Iterator<Item = TokenStream2>,
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
        line,
        tags,
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
        scenario_line: line,
        tags,
        block,
        allow_skipped,
    };
    let case_attrs = examples.map_or_else(Vec::new, |ex| generate_case_attrs(&ex));
    let body = generate_test_tokens(test_config, ctx_prelude, ctx_inserts, ctx_postlude);
    TokenStream::from(quote! {
        #[rstest::rstest]
        #(#case_attrs)*
        #(#attrs)*
        #vis #sig { #body }
    })
}

#[cfg(test)]
mod tests;
