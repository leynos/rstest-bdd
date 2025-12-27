//! Generates rstest-backed test functions for individual scenarios.
//!
//! This module handles the creation of test function signatures, including
//! fixture parameters and scenario outline example parameters, as well as
//! lint suppression for fixtures consumed via `StepContext`.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::collections::HashSet;
use std::path::Path;

use crate::codegen::scenario::{FeaturePath, ScenarioConfig, ScenarioName, generate_scenario_code};
use crate::parsing::examples::ExampleTable;
use crate::parsing::feature::ScenarioData;
use crate::parsing::tags::TagExpression;
use crate::utils::fixtures::extract_function_fixtures;
use crate::utils::ident::sanitize_ident;

use super::macro_args::FixtureSpec;

/// Context for generating a scenario test.
///
/// Captures the sanitised feature file stem for naming tests, the Cargo
/// manifest directory used to render absolute feature paths, the relative
/// path from that manifest to the feature file when embedding diagnostics,
/// an optional tag expression for filtering scenarios, and a slice of fixture
/// specifications to inject into generated test functions.
pub(super) struct ScenarioTestContext<'a> {
    /// Sanitised stem of the feature file used in test function names.
    pub(super) feature_stem: &'a str,
    /// Cargo manifest directory for constructing absolute feature paths.
    pub(super) manifest_dir: &'a Path,
    /// Relative path from `manifest_dir` to the feature file.
    pub(super) rel_path: &'a Path,
    /// Optional tag expression to filter scenarios before test generation.
    pub(super) tag_filter: Option<&'a TagExpression>,
    /// Fixture specifications to inject as parameters in generated tests.
    pub(super) fixtures: &'a [FixtureSpec],
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

/// Builds the test function signature from fixture and example parameters.
fn build_test_signature(
    fn_ident: &syn::Ident,
    fixture_params: &[TokenStream2],
    example_params: &[TokenStream2],
) -> syn::Signature {
    if fixture_params.is_empty() && example_params.is_empty() {
        syn::parse_quote! { fn #fn_ident() }
    } else if fixture_params.is_empty() {
        syn::parse_quote! { fn #fn_ident( #(#example_params),* ) }
    } else if example_params.is_empty() {
        syn::parse_quote! { fn #fn_ident( #(#fixture_params),* ) }
    } else {
        syn::parse_quote! { fn #fn_ident( #(#fixture_params,)* #(#example_params),* ) }
    }
}

/// Generate the test for a single scenario within a feature.
///
/// Derives a unique, rstest-backed function for the scenario using `ctx` to
/// build stable identifiers and feature paths, updating `used_names` to avoid
/// name collisions and returning the resulting `TokenStream2`.
///
/// When `ctx.fixtures` is non-empty, the generated test function includes
/// fixture parameters that rstest resolves via `#[fixture]` functions, and the
/// function is annotated with `#[expect(unused_variables)]` because fixture
/// variables are consumed via `StepContext` rather than referenced directly in
/// the test body.
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
    let mut sig = build_test_signature(&fn_ident, &fixture_params, &example_params);

    let Ok((_args, fixture_setup)) = extract_function_fixtures(&mut sig) else {
        unreachable!("failed to bind fixtures for generated signature");
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
    };
    TokenStream2::from(generate_scenario_code(
        config,
        fixture_setup.prelude.into_iter(),
        fixture_setup.ctx_inserts.into_iter(),
        fixture_setup.postlude.into_iter(),
    ))
}

#[cfg(test)]
mod tests {
    use super::dedupe_name;
    use std::collections::HashSet;

    #[test]
    fn deduplicates_duplicate_titles() {
        let mut used = HashSet::new();
        let first = dedupe_name("dup_same_name", &mut used);
        let second = dedupe_name("dup_same_name", &mut used);
        assert_eq!(first, "dup_same_name");
        assert_eq!(second, "dup_same_name_1");
    }
}
