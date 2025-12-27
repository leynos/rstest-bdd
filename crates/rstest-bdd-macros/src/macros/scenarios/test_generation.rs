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
use crate::parsing::feature::ScenarioData;
use crate::utils::fixtures::extract_function_fixtures;
use crate::utils::ident::sanitize_ident;

use super::macro_args::FixtureSpec;

/// Context for generating a scenario test.
///
/// Captures the sanitised feature file stem for naming tests, the Cargo
/// manifest directory used to render absolute feature paths, and the relative
/// path from that manifest to the feature file when embedding diagnostics.
pub(super) struct ScenarioTestContext<'a> {
    pub(super) feature_stem: &'a str,
    pub(super) manifest_dir: &'a Path,
    pub(super) rel_path: &'a Path,
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

/// Generate the test for a single scenario within a feature.
///
/// Derives a unique, rstest-backed function for the scenario using `ctx` to
/// build stable identifiers and feature paths, updating `used_names` to avoid
/// name collisions and returning the resulting `TokenStream2`.
///
/// When `fixtures` is non-empty, the generated test function includes fixture
/// parameters that rstest resolves via `#[fixture]` functions, and the function
/// is annotated with `#[expect(unused_variables)]` because fixture variables
/// are consumed via `StepContext` rather than referenced directly in the test
/// body.
pub(super) fn generate_scenario_test(
    ctx: &ScenarioTestContext<'_>,
    used_names: &mut HashSet<String>,
    data: ScenarioData,
    fixtures: &[FixtureSpec],
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

    // Add lint suppression when fixtures are present, since fixture variables
    // are consumed via StepContext insertion rather than direct reference.
    let attrs: Vec<syn::Attribute> = if fixtures.is_empty() {
        Vec::new()
    } else {
        vec![syn::parse_quote! {
            #[expect(
                unused_variables,
                reason = "fixture variables are consumed via StepContext, \
                          not referenced directly in the scenario test body"
            )]
        }]
    };

    let vis = syn::Visibility::Inherited;

    // Build fixture parameters for the function signature.
    let fixture_params: Vec<TokenStream2> = fixtures
        .iter()
        .map(|spec| {
            let name = &spec.name;
            let ty = &spec.ty;
            quote! { #name: #ty }
        })
        .collect();

    // Build example parameters for scenario outlines.
    let example_params: Vec<TokenStream2> = examples.as_ref().map_or_else(Vec::new, |ex| {
        ex.headers
            .iter()
            .map(|h| {
                let param_ident = format_ident!("{}", sanitize_ident(h));
                quote! { #[case] #param_ident: &'static str }
            })
            .collect()
    });

    // Combine fixture and example parameters into the function signature.
    let mut sig: syn::Signature = if fixture_params.is_empty() && example_params.is_empty() {
        syn::parse_quote! { fn #fn_ident() }
    } else if fixture_params.is_empty() {
        syn::parse_quote! { fn #fn_ident( #(#example_params),* ) }
    } else if example_params.is_empty() {
        syn::parse_quote! { fn #fn_ident( #(#fixture_params),* ) }
    } else {
        syn::parse_quote! { fn #fn_ident( #(#fixture_params,)* #(#example_params),* ) }
    };

    let Ok((_args, fixture_setup)) = extract_function_fixtures(&mut sig) else {
        unreachable!("failed to bind fixtures for generated signature");
    };
    let ctx_prelude = fixture_setup.prelude;
    let ctx_inserts = fixture_setup.ctx_inserts;
    let ctx_postlude = fixture_setup.postlude;
    let block: syn::Block = syn::parse_quote!({});

    let feature_path = ctx.manifest_dir.join(ctx.rel_path).display().to_string();

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
        ctx_prelude.into_iter(),
        ctx_inserts.into_iter(),
        ctx_postlude.into_iter(),
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
