//! Generates rstest-backed test functions for individual scenarios.
//!
//! This module handles the creation of test function signatures, including
//! fixture parameters and scenario outline example parameters, as well as
//! lint suppression for fixtures consumed via `StepContext`.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::collections::HashSet;
use std::path::Path;

use crate::codegen::scenario::{
    FeaturePath, ScenarioConfig, ScenarioName, ScenarioReturnKind, generate_scenario_code,
};
use crate::parsing::examples::ExampleTable;
use crate::parsing::feature::ScenarioData;
use crate::parsing::tags::TagExpression;
use crate::utils::fixtures::extract_function_fixtures;
use crate::utils::ident::sanitize_ident;

use super::macro_args::{FixtureSpec, RuntimeCompatibilityAlias, RuntimeMode};

/// Context for generating a scenario test, capturing the feature file stem,
/// manifest directory, relative path, tag filter, fixtures, runtime mode,
/// and optional harness/attributes paths.
pub(super) struct ScenarioTestContext<'a> {
    /// Sanitized stem of the feature file used in test function names.
    pub(super) feature_stem: &'a str,
    /// Cargo manifest directory for constructing absolute feature paths.
    pub(super) manifest_dir: &'a Path,
    /// Relative path from `manifest_dir` to the feature file.
    pub(super) rel_path: &'a Path,
    /// Optional tag expression to filter scenarios before test generation.
    pub(super) tag_filter: Option<&'a TagExpression>,
    /// Fixture specifications to inject as parameters in generated tests.
    pub(super) fixtures: &'a [FixtureSpec],
    /// Runtime mode for test execution (sync or async/Tokio).
    pub(super) runtime: RuntimeMode,
    /// Optional compatibility alias inferred from legacy runtime syntax.
    pub(super) runtime_alias: Option<RuntimeCompatibilityAlias>,
    /// Optional harness adapter type for compile-time trait assertion.
    pub(super) harness: Option<&'a syn::Path>,
    /// Optional attribute policy type for compile-time trait assertion.
    pub(super) attributes: Option<&'a syn::Path>,
}

/// Resolves which harness path should be used for code generation.
///
/// Runtime compatibility aliases are tracked so `runtime = "tokio-current-thread"`
/// can be treated as a harness-selection alias internally. Until the dedicated
/// Tokio harness plug-in crate ships (phase 9.3), legacy runtime mode continues
/// to use the existing runtime code path when no explicit harness is provided.
fn resolve_harness_path(
    explicit_harness: Option<&syn::Path>,
    runtime_alias: Option<RuntimeCompatibilityAlias>,
) -> Option<&syn::Path> {
    if explicit_harness.is_some() {
        return explicit_harness;
    }
    if runtime_alias.is_some() {
        // Compatibility alias is recognized and reserved for future harness wiring.
        return None;
    }
    None
}

pub(super) fn dedupe_name(base: &str, used: &mut HashSet<String>) -> String {
    let mut name = base.to_string();
    let mut counter = 1usize;
    while used.contains(&name) {
        name = format!("{base}_{counter}");
        counter += 1;
    }
    used.insert(name.clone());
    name
}

/// Builds lint suppression attributes when fixtures are present.
fn build_lint_attributes(fixtures: &[FixtureSpec]) -> Vec<syn::Attribute> {
    if fixtures.is_empty() {
        Vec::new()
    } else {
        vec![syn::parse_quote! {
            #[expect(
                unused_variables,
                reason = "fixture variables are consumed via StepContext, \
                          not referenced directly in the scenario test body"
            )]
        }]
    }
}

/// Builds fixture parameters for the test function signature.
fn build_fixture_params(fixtures: &[FixtureSpec]) -> Vec<TokenStream2> {
    fixtures
        .iter()
        .map(|spec| {
            let name = &spec.name;
            let ty = &spec.ty;
            quote! { #name: #ty }
        })
        .collect()
}

/// Builds example parameters for scenario outline test functions.
fn build_example_params(examples: Option<&ExampleTable>) -> Vec<TokenStream2> {
    examples.map_or_else(Vec::new, |ex| {
        ex.headers
            .iter()
            .map(|h| {
                let param_ident = format_ident!("{}", sanitize_ident(h));
                quote! { #[case] #param_ident: &'static str }
            })
            .collect()
    })
}

/// Combines fixture and example parameters into a single token stream.
fn combine_params(
    fixture_params: &[TokenStream2],
    example_params: &[TokenStream2],
) -> TokenStream2 {
    match (fixture_params.is_empty(), example_params.is_empty()) {
        (true, true) => quote! {},
        (true, false) => quote! { #(#example_params),* },
        (false, true) => quote! { #(#fixture_params),* },
        (false, false) => quote! { #(#fixture_params,)* #(#example_params),* },
    }
}

/// Builds the test function signature from fixture and example parameters.
fn build_test_signature(
    fn_ident: &syn::Ident,
    fixture_params: &[TokenStream2],
    example_params: &[TokenStream2],
    is_async: bool,
) -> syn::Signature {
    let params = combine_params(fixture_params, example_params);
    if is_async {
        syn::parse_quote! { async fn #fn_ident(#params) }
    } else {
        syn::parse_quote! { fn #fn_ident(#params) }
    }
}

/// Generate an rstest-backed test for a single scenario within a feature.
///
/// Derives a unique function using `ctx` to build stable identifiers and
/// feature paths, updating `used_names` to avoid collisions. When
/// `ctx.fixtures` is non-empty, the generated function includes fixture
/// parameters resolved via `#[fixture]` and `#[expect(unused_variables)]`
/// because fixture variables are consumed via `StepContext`.
pub(super) fn generate_scenario_test(
    ctx: &ScenarioTestContext<'_>,
    used_names: &mut HashSet<String>,
    data: ScenarioData,
) -> TokenStream2 {
    let ScenarioData {
        name,
        steps,
        examples,
        tags,
        line,
    } = data;
    let allow_skipped = crate::codegen::scenario::scenario_allows_skip(&tags);
    let base_name = format!("{}_{}", ctx.feature_stem, sanitize_ident(&name));
    let fn_name = dedupe_name(&base_name, used_names);
    let fn_ident = format_ident!("{}", fn_name);

    let attrs = build_lint_attributes(ctx.fixtures);
    let fixture_params = build_fixture_params(ctx.fixtures);
    let example_params = build_example_params(examples.as_ref());
    let is_async = ctx.runtime.is_async();
    let mut sig = build_test_signature(&fn_ident, &fixture_params, &example_params, is_async);

    let (_args, fixture_setup) = match extract_function_fixtures(&mut sig) {
        Ok(result) => result,
        Err(err) => return err.to_compile_error(),
    };

    let feature_path = ctx.manifest_dir.join(ctx.rel_path).display().to_string();
    let vis = syn::Visibility::Inherited;
    let block: syn::Block = syn::parse_quote!({});

    let config = ScenarioConfig {
        attrs: &attrs,
        vis: &vis,
        sig: &sig,
        block: &block,
        feature_path: FeaturePath::new(feature_path),
        scenario_name: ScenarioName::new(name),
        steps,
        examples,
        allow_skipped,
        line,
        tags: &tags,
        runtime: ctx.runtime,
        return_kind: ScenarioReturnKind::Unit,
        harness: resolve_harness_path(ctx.harness, ctx.runtime_alias),
        attributes: ctx.attributes,
    };
    TokenStream2::from(generate_scenario_code(
        &config,
        fixture_setup.prelude.into_iter(),
        fixture_setup.ctx_inserts.into_iter(),
        fixture_setup.postlude.into_iter(),
    ))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code uses infallible unwraps and indexed access for clarity"
)]
mod tests;
