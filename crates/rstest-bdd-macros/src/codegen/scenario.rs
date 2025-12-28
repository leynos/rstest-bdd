//! Code generation for scenario tests.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

mod domain;
mod helpers;
mod metadata;
mod runtime;

pub(crate) use domain::*;

pub(crate) use helpers::process_steps;
use helpers::{generate_case_attrs, generate_indexed_case_attrs, process_steps_substituted};
pub(crate) use metadata::{FeaturePath, ScenarioName};
use runtime::{
    OutlineTestTokensConfig, ProcessedSteps, TestTokensConfig, generate_test_tokens,
    generate_test_tokens_outline,
};

use crate::parsing::placeholder::contains_placeholders;

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

/// Checks if any step in the scenario contains placeholder tokens.
fn steps_contain_placeholders(steps: &[crate::parsing::feature::ParsedStep]) -> bool {
    steps.iter().any(|step| {
        contains_placeholders(&step.text)
            || step
                .docstring
                .as_ref()
                .is_some_and(|d| contains_placeholders(d))
            || step.table.as_ref().is_some_and(|t| {
                t.iter()
                    .any(|row| row.iter().any(|cell| contains_placeholders(cell)))
            })
    })
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

    // Check if this is a scenario outline with placeholders in steps
    let is_outline_with_placeholders = examples.is_some() && steps_contain_placeholders(&steps);

    let func_meta = FunctionMetadata {
        attrs,
        vis,
        sig,
        block,
    };
    let scenario_meta = ScenarioMeta {
        feature_path: &feature_path,
        scenario_name: &scenario_name,
        steps: &steps,
        allow_skipped,
        line,
        tags,
    };
    let ctx_iterators = ContextIterators {
        prelude: ctx_prelude,
        inserts: ctx_inserts,
        postlude: ctx_postlude,
    };

    // Use match to ensure examples is available for outline without expect
    match (is_outline_with_placeholders, examples.as_ref()) {
        (true, Some(ex)) => {
            generate_outline_scenario_code(&func_meta, &scenario_meta, ex, ctx_iterators)
        }
        _ => generate_regular_scenario_code(
            &func_meta,
            &scenario_meta,
            examples.as_ref(),
            ctx_iterators,
        ),
    }
}

/// Function metadata from the annotated test function.
pub(crate) struct FunctionMetadata<'a> {
    pub(crate) attrs: &'a [syn::Attribute],
    pub(crate) vis: &'a syn::Visibility,
    pub(crate) sig: &'a syn::Signature,
    pub(crate) block: &'a syn::Block,
}

/// Context iterators for test generation.
pub(crate) struct ContextIterators<P, I, T> {
    pub(crate) prelude: P,
    pub(crate) inserts: I,
    pub(crate) postlude: T,
}

/// Scenario metadata for code generation.
struct ScenarioMeta<'a> {
    feature_path: &'a FeaturePath,
    scenario_name: &'a ScenarioName,
    steps: &'a [crate::parsing::feature::ParsedStep],
    allow_skipped: bool,
    line: u32,
    tags: &'a [String],
}

/// Generate code for a regular scenario (no placeholder substitution).
fn generate_regular_scenario_code(
    func_meta: &FunctionMetadata<'_>,
    scenario_meta: &ScenarioMeta<'_>,
    examples: Option<&crate::parsing::examples::ExampleTable>,
    ctx_iterators: ContextIterators<
        impl Iterator<Item = TokenStream2>,
        impl Iterator<Item = TokenStream2>,
        impl Iterator<Item = TokenStream2>,
    >,
) -> TokenStream {
    let FunctionMetadata {
        attrs,
        vis,
        sig,
        block,
    } = func_meta;
    let ScenarioMeta {
        feature_path,
        scenario_name,
        steps,
        allow_skipped,
        line,
        tags,
    } = scenario_meta;
    let ContextIterators {
        prelude: ctx_prelude,
        inserts: ctx_inserts,
        postlude: ctx_postlude,
    } = ctx_iterators;
    let (keyword_tokens, values, docstrings, tables) = process_steps(steps);
    debug_assert_eq!(keyword_tokens.len(), steps.len());
    let processed_steps = ProcessedSteps {
        keyword_tokens,
        values,
        docstrings,
        tables,
    };
    let test_config = TestTokensConfig {
        processed_steps,
        feature_path,
        scenario_name,
        scenario_line: *line,
        tags,
        block,
        allow_skipped: *allow_skipped,
    };
    let case_attrs = examples.map_or_else(Vec::new, generate_case_attrs);
    let body = generate_test_tokens(test_config, ctx_prelude, ctx_inserts, ctx_postlude);
    TokenStream::from(quote! {
        #[rstest::rstest]
        #(#case_attrs)*
        #(#attrs)*
        #vis #sig { #body }
    })
}

/// Generate code for a scenario outline with placeholder substitution.
fn generate_outline_scenario_code(
    func_meta: &FunctionMetadata<'_>,
    scenario_meta: &ScenarioMeta<'_>,
    examples: &crate::parsing::examples::ExampleTable,
    ctx_iterators: ContextIterators<
        impl Iterator<Item = TokenStream2>,
        impl Iterator<Item = TokenStream2>,
        impl Iterator<Item = TokenStream2>,
    >,
) -> TokenStream {
    let FunctionMetadata {
        attrs,
        vis,
        sig,
        block,
    } = func_meta;
    let ScenarioMeta {
        feature_path,
        scenario_name,
        steps,
        allow_skipped,
        line,
        tags,
    } = scenario_meta;
    let ContextIterators {
        prelude: ctx_prelude,
        inserts: ctx_inserts,
        postlude: ctx_postlude,
    } = ctx_iterators;

    // Generate substituted steps for each Examples row
    let headers = ExampleHeaders::new(examples.headers.clone());
    let all_rows_steps: Result<Vec<_>, _> = examples
        .rows
        .iter()
        .filter(|row| row.iter().any(|cell| !cell.is_empty()))
        .map(|row| {
            let row = ExampleRow::new(row.clone());
            process_steps_substituted(steps, &headers, &row)
        })
        .collect();

    let all_rows_steps = match all_rows_steps {
        Ok(steps) => steps,
        Err(err) => return TokenStream::from(err),
    };

    let outline_config = OutlineTestTokensConfig {
        all_rows_steps,
        feature_path,
        scenario_name,
        scenario_line: *line,
        tags,
        block,
        allow_skipped: *allow_skipped,
    };

    let case_attrs = generate_indexed_case_attrs(examples);
    let body = generate_test_tokens_outline(outline_config, ctx_prelude, ctx_inserts, ctx_postlude);

    // Add the hidden case index parameter to the signature
    let mut modified_sig = (*sig).clone();
    let case_idx_param: syn::FnArg = syn::parse_quote! {
        #[case] __rstest_bdd_case_idx: usize
    };
    modified_sig.inputs.insert(0, case_idx_param);

    TokenStream::from(quote! {
        #[rstest::rstest]
        #(#case_attrs)*
        #(#attrs)*
        #vis #modified_sig { #body }
    })
}

#[cfg(test)]
mod tests;
