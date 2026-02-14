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

use super::macro_args::{FixtureSpec, RuntimeMode};

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
    /// Optional harness adapter type for compile-time trait assertion.
    pub(super) harness: Option<&'a syn::Path>,
    /// Optional attribute policy type for compile-time trait assertion.
    pub(super) attributes: Option<&'a syn::Path>,
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
        harness: ctx.harness,
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
mod tests {
    use super::FixtureSpec;
    use super::{build_fixture_params, build_lint_attributes, build_test_signature, dedupe_name};
    use proc_macro2::TokenStream as TokenStream2;
    use quote::quote;
    use std::collections::HashSet;

    #[test]
    fn deduplicates_duplicate_titles() {
        let mut used = HashSet::new();
        let first = dedupe_name("dup_same_name", &mut used);
        let second = dedupe_name("dup_same_name", &mut used);
        assert_eq!(first, "dup_same_name");
        assert_eq!(second, "dup_same_name_1");
    }

    fn make_fixture_spec(name: &str, ty: &str) -> FixtureSpec {
        FixtureSpec {
            name: syn::parse_str(name).unwrap(),
            ty: syn::parse_str(ty).unwrap(),
        }
    }

    fn sig_to_string(sig: &syn::Signature) -> String {
        quote!(#sig).to_string()
    }

    // Tests for build_lint_attributes

    #[test]
    fn build_lint_attributes_empty_fixtures_produces_no_attributes() {
        let attrs = build_lint_attributes(&[]);
        assert!(attrs.is_empty());
    }

    #[test]
    fn build_lint_attributes_with_fixtures_produces_expect_attribute() {
        let fixtures = vec![make_fixture_spec("world", "TestWorld")];
        let attrs = build_lint_attributes(&fixtures);

        assert_eq!(attrs.len(), 1);
        let attr = &attrs[0];

        // Verify it's an #[expect(...)] attribute
        assert!(attr.path().is_ident("expect"));

        // Verify the attribute contains unused_variables
        let attr_str = quote!(#attr).to_string();
        assert!(
            attr_str.contains("unused_variables"),
            "attribute should contain unused_variables: {attr_str}"
        );
        assert!(
            attr_str.contains("reason"),
            "attribute should contain reason: {attr_str}"
        );
        assert!(
            attr_str.contains("StepContext"),
            "reason should mention StepContext: {attr_str}"
        );
    }

    #[test]
    fn build_lint_attributes_multiple_fixtures_still_produces_single_attribute() {
        let fixtures = vec![
            make_fixture_spec("world", "TestWorld"),
            make_fixture_spec("db", "Database"),
        ];
        let attrs = build_lint_attributes(&fixtures);
        assert_eq!(attrs.len(), 1);
    }

    // Tests for build_test_signature

    #[test]
    fn build_test_signature_no_fixtures_no_examples() {
        let fn_ident = syn::Ident::new("test_name", proc_macro2::Span::call_site());
        let sig = build_test_signature(&fn_ident, &[], &[], false);
        assert_eq!(sig_to_string(&sig), "fn test_name ()");
    }

    #[test]
    fn build_test_signature_async_no_fixtures_no_examples() {
        let fn_ident = syn::Ident::new("test_name", proc_macro2::Span::call_site());
        let sig = build_test_signature(&fn_ident, &[], &[], true);
        assert_eq!(sig_to_string(&sig), "async fn test_name ()");
    }

    #[test]
    fn build_test_signature_fixtures_only() {
        let fn_ident = syn::Ident::new("test_name", proc_macro2::Span::call_site());
        let fixture_params: Vec<TokenStream2> = vec![quote!(f1: T1), quote!(f2: T2)];

        let sig = build_test_signature(&fn_ident, &fixture_params, &[], false);
        let sig_str = sig_to_string(&sig);

        assert!(sig_str.contains("f1 : T1"), "should contain f1: T1");
        assert!(sig_str.contains("f2 : T2"), "should contain f2: T2");
    }

    #[test]
    fn build_test_signature_async_fixtures_only() {
        let fn_ident = syn::Ident::new("test_name", proc_macro2::Span::call_site());
        let fixture_params: Vec<TokenStream2> = vec![quote!(f1: T1)];

        let sig = build_test_signature(&fn_ident, &fixture_params, &[], true);
        let sig_str = sig_to_string(&sig);

        assert!(sig_str.starts_with("async fn"), "should be async fn");
        assert!(sig_str.contains("f1 : T1"), "should contain f1: T1");
    }

    #[test]
    fn build_test_signature_examples_only() {
        let fn_ident = syn::Ident::new("test_name", proc_macro2::Span::call_site());
        let example_params: Vec<TokenStream2> = vec![
            quote!(#[case] col1: &'static str),
            quote!(#[case] col2: &'static str),
        ];

        let sig = build_test_signature(&fn_ident, &[], &example_params, false);
        let sig_str = sig_to_string(&sig);

        assert!(sig_str.contains("# [case]"), "should contain #[case]");
        assert!(sig_str.contains("col1"), "should contain col1");
        assert!(sig_str.contains("col2"), "should contain col2");
    }

    #[test]
    fn build_test_signature_fixtures_then_examples() {
        let fn_ident = syn::Ident::new("test_name", proc_macro2::Span::call_site());
        let fixture_params: Vec<TokenStream2> = vec![quote!(world: TestWorld)];
        let example_params: Vec<TokenStream2> = vec![
            quote!(#[case] col1: &'static str),
            quote!(#[case] col2: &'static str),
        ];

        let sig = build_test_signature(&fn_ident, &fixture_params, &example_params, false);
        let sig_str = sig_to_string(&sig);

        // Fixtures must come first, followed by #[case] example parameters
        let world_pos = sig_str.find("world").expect("should contain world");
        let col1_pos = sig_str.find("col1").expect("should contain col1");
        assert!(
            world_pos < col1_pos,
            "fixture 'world' should appear before example 'col1'"
        );
    }

    #[test]
    fn build_test_signature_async_fixtures_then_examples() {
        let fn_ident = syn::Ident::new("test_name", proc_macro2::Span::call_site());
        let fixture_params: Vec<TokenStream2> = vec![quote!(world: TestWorld)];
        let example_params: Vec<TokenStream2> = vec![
            quote!(#[case] col1: &'static str),
            quote!(#[case] col2: &'static str),
        ];

        let sig = build_test_signature(&fn_ident, &fixture_params, &example_params, true);
        let sig_str = sig_to_string(&sig);

        // Must be async function
        assert!(sig_str.starts_with("async fn"), "should be async fn");

        // Fixtures must come first, followed by #[case] example parameters
        let world_pos = sig_str.find("world").expect("should contain world");
        let col1_pos = sig_str.find("col1").expect("should contain col1");
        assert!(
            world_pos < col1_pos,
            "fixture 'world' should appear before example 'col1'"
        );
    }

    // Tests for build_fixture_params

    #[test]
    fn build_fixture_params_empty() {
        let params = build_fixture_params(&[]);
        assert!(params.is_empty());
    }

    #[test]
    fn build_fixture_params_single() {
        let fixtures = vec![make_fixture_spec("world", "TestWorld")];
        let params = build_fixture_params(&fixtures);

        assert_eq!(params.len(), 1);
        let param_str = params[0].to_string();
        assert!(param_str.contains("world"));
        assert!(param_str.contains("TestWorld"));
    }

    #[test]
    fn build_fixture_params_multiple() {
        let fixtures = vec![
            make_fixture_spec("world", "TestWorld"),
            make_fixture_spec("db", "Database"),
        ];
        let params = build_fixture_params(&fixtures);

        assert_eq!(params.len(), 2);
    }
}
