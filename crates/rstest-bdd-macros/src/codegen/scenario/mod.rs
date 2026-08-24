//! Code generation for scenario tests.
//!
//! This module coordinates the full code-generation pipeline for a single
//! BDD scenario or scenario outline. The pipeline is partitioned across
//! six focused sub-modules:
//!
//! - [`adapters`] — owns adapter API resolution and the corresponding fallback
//!   diagnostics for an expansion boundary, so generated scenarios reuse one
//!   decision.
//!
//! - [`domain`] — domain types shared across the pipeline (`StepText`,
//!   `ExampleHeaders`, `ExampleRow`, and `Docstring`).
//! - [`helpers`] — step-processing utilities and case-attribute generators.
//! - [`metadata`] — strongly-typed wrappers for feature-path and
//!   scenario-name values used in generated code.
//! - [`runtime`] — token generation for the async runtime wrapper and the
//!   harness-orchestrated `ScenarioRunRequest`.
//! - [`test_attrs`] — ADR-008 attribute-policy resolution, translating
//!   harness and runtime-mode hints into the correct set of test attributes
//!   (`#[rstest::rstest]`, `#[tokio::test]`, `#[gpui::test]`).
//!
//! Public entry points are [`generate_scenario`] and
//! [`generate_scenario_outline`], which delegate to the internal helpers
//! after resolving adapter API paths once at the expansion boundary.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::borrow::Cow;

mod adapters;
mod boundary;
mod domain;
mod helpers;
mod metadata;
mod runtime;
mod test_attrs;

use adapters::resolve_scenario_adapters;
use boundary::finalize_scenario_signature;
pub(crate) use domain::*;
pub(crate) use helpers::process_steps;
use helpers::{
    generate_case_attrs, generate_indexed_case_attrs, process_steps_substituted, row_has_values,
};
pub(crate) use metadata::{FeaturePath, ScenarioName};
use runtime::{
    OutlineTestTokensConfig, ProcessedSteps, ScenarioMetadata, TestTokensConfig,
    generate_test_tokens, generate_test_tokens_outline,
};

pub(crate) use crate::macros::scenarios::ScenariosRuntimeMode as RuntimeMode;
use crate::macros::scenarios::ScenariosTestAttributeHint as TestAttributeHint;

use crate::parsing::placeholder::contains_placeholders;

/// Return kinds supported by scenario bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScenarioReturnKind {
    Unit,
    ResultUnit,
}

impl ScenarioReturnKind {
    pub(crate) fn is_fallible(self) -> bool {
        matches!(self, Self::ResultUnit)
    }
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
    /// Runtime mode for test execution (sync or async/Tokio).
    pub(crate) runtime: RuntimeMode,
    /// Runtime mode used when resolving generated test attributes.
    pub(crate) attribute_runtime: RuntimeMode,
    /// Return shape expected from the scenario body.
    pub(crate) return_kind: ScenarioReturnKind,
    /// Optional harness adapter type path for compile-time trait assertion.
    pub(crate) harness: Option<&'a syn::Path>,
    /// Optional attribute policy type path for compile-time trait assertion.
    pub(crate) attributes: Option<&'a syn::Path>,
    /// Boundary resolution; direct code-generation tests may use the pure local resolver.
    pub(crate) resolutions: Option<&'a crate::codegen::SharedAdapterResolutions>,
    /// Boundary tokens for `#[scenario]`; `scenarios!` emits around its module.
    pub(crate) fallback_diagnostics: Option<&'a TokenStream2>,
}

/// Configuration for context iterators in scenario code generation.
pub(crate) struct ContextConfig<P, I, Q> {
    pub(crate) prelude: P,
    pub(crate) inserts: I,
    pub(crate) postlude: Q,
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
    config: &ScenarioConfig<'_>,
    ctx_prelude: impl Iterator<Item = TokenStream2>,
    ctx_inserts: impl Iterator<Item = TokenStream2>,
    ctx_postlude: impl Iterator<Item = TokenStream2>,
) -> TokenStream2 {
    // Check if this is a scenario outline with placeholders in steps
    let is_outline_with_placeholders =
        config.examples.is_some() && steps_contain_placeholders(&config.steps);
    let ctx = ContextConfig {
        prelude: ctx_prelude,
        inserts: ctx_inserts,
        postlude: ctx_postlude,
    };

    if is_outline_with_placeholders {
        generate_outline_scenario_code(config, ctx)
    } else {
        generate_regular_scenario_code(config, ctx)
    }
}

/// Reject unsupported combinations of a harness and an async scenario.
fn reject_async_harness(config: &ScenarioConfig<'_>) -> Option<TokenStream2> {
    if config.harness.is_none() || !config.runtime.is_async() {
        return None;
    }

    let err = syn::Error::new(
        proc_macro2::Span::call_site(),
        "combining `harness` with `async fn` scenarios is not supported; \
         use a synchronous scenario function with `TokioHarness` instead \
         (the harness provides the Tokio runtime for step functions)",
    );
    Some(err.into_compile_error())
}

/// Generate code for a regular scenario (no placeholder substitution).
fn generate_regular_scenario_code<P, I, Q>(
    config: &ScenarioConfig<'_>,
    ctx: ContextConfig<P, I, Q>,
) -> TokenStream2
where
    P: Iterator<Item = TokenStream2>,
    I: Iterator<Item = TokenStream2>,
    Q: Iterator<Item = TokenStream2>,
{
    if let Some(rejection) = reject_async_harness(config) {
        return rejection;
    }

    let (keyword_tokens, values, docstrings, tables) = process_steps(&config.steps);
    debug_assert_eq!(keyword_tokens.len(), config.steps.len());
    let processed_steps = ProcessedSteps {
        keyword_tokens,
        values,
        docstrings,
        tables,
    };
    let adapters = resolve_scenario_adapters(config);
    let harness_resolution = adapters.resolutions.harness.as_ref();
    let attributes_resolution = adapters.resolutions.attributes.as_ref();
    let metadata = ScenarioMetadata {
        feature_path: &config.feature_path,
        scenario_name: &config.scenario_name,
        scenario_line: config.line,
        tags: config.tags,
        block: config.block,
        allow_skipped: config.allow_skipped,
        is_async: config.runtime.is_async(),
        return_kind: config.return_kind,
        harness: config.harness,
        harness_api_path: harness_resolution.map(|resolution| resolution.api_path.clone()),
    };
    let test_config = TestTokensConfig {
        processed_steps,
        metadata,
    };
    let case_attrs = config
        .examples
        .as_ref()
        .map_or_else(Vec::new, generate_case_attrs);
    let body = generate_test_tokens(&test_config, ctx.prelude, ctx.inserts, ctx.postlude);
    let attrs = config.attrs;
    let vis = config.vis;
    let mut signature = Cow::Borrowed(config.sig);
    let (trait_assertions, test_attrs, underscore_expect, body) = finalize_scenario_signature(
        config,
        harness_resolution,
        attributes_resolution,
        &mut signature,
        body,
    );
    let fallback_diagnostics = config.fallback_diagnostics;
    quote! {
        #fallback_diagnostics
        #trait_assertions
        #test_attrs
        #(#case_attrs)*
        #(#attrs)*
        #underscore_expect
        #vis #signature { #body }
    }
}

/// Generate code for a scenario outline with placeholder substitution.
fn generate_outline_scenario_code<P, I, Q>(
    config: &ScenarioConfig<'_>,
    ctx: ContextConfig<P, I, Q>,
) -> TokenStream2
where
    P: Iterator<Item = TokenStream2>,
    I: Iterator<Item = TokenStream2>,
    Q: Iterator<Item = TokenStream2>,
{
    if let Some(rejection) = reject_async_harness(config) {
        return rejection;
    }

    // Generate substituted steps for each Examples row
    let Some(examples) = config.examples.as_ref() else {
        let err = syn::Error::new(
            proc_macro2::Span::call_site(),
            "Scenario outline examples missing",
        );
        return err.into_compile_error();
    };
    let headers = ExampleHeaders::new(examples.headers.clone());
    let all_rows_steps: Result<Vec<_>, _> = examples
        .rows
        .iter()
        .filter(|row| row_has_values(row))
        .map(|row| {
            let row = ExampleRow::new(row.clone());
            process_steps_substituted(&config.steps, &headers, &row)
        })
        .collect();

    let all_rows_steps = match all_rows_steps {
        Ok(steps) => steps,
        Err(err) => return err,
    };

    let adapters = resolve_scenario_adapters(config);
    let harness_resolution = adapters.resolutions.harness.as_ref();
    let attributes_resolution = adapters.resolutions.attributes.as_ref();
    let metadata = ScenarioMetadata {
        feature_path: &config.feature_path,
        scenario_name: &config.scenario_name,
        scenario_line: config.line,
        tags: config.tags,
        block: config.block,
        allow_skipped: config.allow_skipped,
        is_async: config.runtime.is_async(),
        return_kind: config.return_kind,
        harness: config.harness,
        harness_api_path: harness_resolution.map(|resolution| resolution.api_path.clone()),
    };
    let outline_config = OutlineTestTokensConfig {
        all_rows_steps,
        metadata,
    };

    let case_attrs = generate_indexed_case_attrs(examples);
    let body =
        generate_test_tokens_outline(&outline_config, ctx.prelude, ctx.inserts, ctx.postlude);

    // Add the hidden case index parameter to the signature
    let mut signature: Cow<'_, syn::Signature> = Cow::Owned((*config.sig).clone());
    let case_idx_param: syn::FnArg = syn::parse_quote! {
        #[case] __rstest_bdd_case_idx: usize
    };
    signature.to_mut().inputs.insert(0, case_idx_param);

    let attrs = config.attrs;
    let vis = config.vis;
    let (trait_assertions, test_attrs, underscore_expect, body) = finalize_scenario_signature(
        config,
        harness_resolution,
        attributes_resolution,
        &mut signature,
        body,
    );
    let fallback_diagnostics = config.fallback_diagnostics;
    quote! {
        #fallback_diagnostics
        #trait_assertions
        #test_attrs
        #(#case_attrs)*
        #(#attrs)*
        #underscore_expect
        #vis #signature { #body }
    }
}

#[cfg(test)]
mod tests;
