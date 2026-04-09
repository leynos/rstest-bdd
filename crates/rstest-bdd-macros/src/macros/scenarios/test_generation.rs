//! Generates rstest-backed test functions for individual scenarios.
//!
//! This module handles the creation of test function signatures, including
//! fixture parameters and scenario outline example parameters, as well as
//! lint suppression for fixtures consumed via `StepContext`.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::collections::HashSet;
use std::path::Path;

use crate::codegen::rstest_bdd_harness_tokio_path;
use crate::codegen::scenario::{
    FeaturePath, ScenarioConfig, ScenarioName, ScenarioReturnKind, generate_scenario_code,
};
use crate::parsing::examples::ExampleTable;
use crate::parsing::feature::ScenarioData;
use crate::parsing::tags::TagExpression;
use crate::return_classifier::classify_return_type;
use crate::utils::fixtures::extract_function_fixtures;
use crate::utils::ident::sanitize_ident;
use crate::utils::result_type::try_extract_result_error_type;

use super::macro_args::{
    FixtureSpec, RuntimeCompatibilityAlias, RuntimeMode, runtime_compatibility_alias,
};

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

/// Resolves which harness path should be used for code generation.
///
/// Runtime compatibility aliases map legacy runtime syntax to harness selection.
/// The `runtime = "tokio-current-thread"` compatibility alias now resolves to
/// `TokioHarness` from the `rstest-bdd-harness-tokio` crate (activated in roadmap
/// item 9.2.4). The crate path is resolved via `proc_macro_crate` to support
/// downstream crates that rename the dependency in their `Cargo.toml`. Explicit
/// harness selection takes precedence over compatibility aliases. See design doc
/// §2.5.5 and §2.7.3.
///
/// Returns an owned `syn::Path` because the alias branch constructs a new path
/// at macro-expansion time. The explicit-harness branch clones the reference;
/// this allocation is negligible during proc-macro expansion, which is not a
/// hot path.
///
/// # Examples
/// ```rust,ignore
/// # use syn::parse_quote;
/// let explicit: syn::Path = parse_quote!(MyHarness);
/// assert!(resolve_harness_path(Some(&explicit), None).is_some());
/// let resolved = resolve_harness_path(
///     None,
///     Some(RuntimeCompatibilityAlias::TokioHarnessAdapter),
/// );
/// assert!(resolved.is_some());
/// ```
fn resolve_harness_path(
    explicit_harness: Option<&syn::Path>,
    runtime_alias: Option<RuntimeCompatibilityAlias>,
) -> Option<syn::Path> {
    if let Some(path) = explicit_harness {
        return Some(path.clone());
    }
    if let Some(RuntimeCompatibilityAlias::TokioHarnessAdapter) = runtime_alias {
        // Resolve the crate path via proc_macro_crate, then append ::TokioHarness.
        let crate_path = rstest_bdd_harness_tokio_path();
        return Some(syn::parse_quote!(#crate_path::TokioHarness));
    }
    None
}

/// Determines the effective runtime mode for code generation.
///
/// When a concrete runtime compatibility alias is active and no explicit
/// harness was provided, the alias drives the runtime mode to synchronous.
/// Checking the alias variant directly (rather than the resolved harness
/// presence) ensures future harness/alias combinations don't accidentally
/// get forced to sync.
fn resolve_effective_runtime(
    runtime: RuntimeMode,
    alias: Option<RuntimeCompatibilityAlias>,
    explicit_harness: Option<&syn::Path>,
) -> RuntimeMode {
    if matches!(alias, Some(RuntimeCompatibilityAlias::TokioHarnessAdapter))
        && explicit_harness.is_none()
    {
        RuntimeMode::Sync
    } else {
        runtime
    }
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

/// Normalizes a type for structural comparison by converting it to a
/// canonical string representation with consistent formatting.
///
/// This is used for deduplicating error types where `std::io::Error` and
/// `Error` (when both refer to the same type via imports) should be
/// considered distinct at the syntax level, but semantically equivalent
/// paths like `::std::io::Error` and `std::io::Error` should match.
fn normalize_type_key(ty: &syn::Type) -> String {
    // Use quote! to get a consistent token representation, then normalize
    // whitespace and remove leading :: for comparison
    let rendered = quote!(#ty).to_string();
    rendered
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_start_matches("::")
        .to_string()
}

/// Resolves a unified error type from `Result`-typed fixture specifications.
///
/// When all `Result<T, E>` fixtures share the same error type `E`, returns
/// that type directly so the generated test signature uses it verbatim.
/// When fixtures use different error types (or none are Result-typed), falls
/// back to `Box<dyn ::std::error::Error>`.
fn resolve_fixture_error_type(fixtures: &[FixtureSpec]) -> syn::Type {
    let mut error_types: Vec<syn::Type> = fixtures
        .iter()
        .filter_map(|spec| try_extract_result_error_type(&spec.ty))
        .collect();
    error_types.dedup_by(|a, b| normalize_type_key(a) == normalize_type_key(b));
    if error_types.len() == 1 {
        error_types.remove(0)
    } else {
        syn::parse_quote! { Box<dyn ::std::error::Error> }
    }
}

/// Classifies the scenario return kind from the current signature output,
/// upgrading to `ResultUnit` when Result-returning fixtures require error
/// propagation and the signature is not already fallible.
///
/// Uses [`classify_return_type`] to determine the initial [`ScenarioReturnKind`]:
/// `Unit` for unit returns, `ResultUnit` for fallible returns. When
/// `has_result_fixtures` is true and the return kind is not already fallible,
/// this function **mutates** `sig.output` in-place via
/// [`resolve_fixture_error_type`] to upgrade it to `Result<(), E>`, ensuring
/// the generated function signature can propagate fixture initialization errors.
fn resolve_scenario_return_kind(
    sig: &mut syn::Signature,
    has_result_fixtures: bool,
    fixtures: &[FixtureSpec],
) -> ScenarioReturnKind {
    let mut return_kind = classify_return_type(&sig.output, None)
        .map(|rk| match rk {
            crate::return_classifier::ReturnKind::Unit => ScenarioReturnKind::Unit,
            _ => ScenarioReturnKind::ResultUnit,
        })
        .unwrap_or(ScenarioReturnKind::Unit);

    if has_result_fixtures && !return_kind.is_fallible() {
        let error_ty = resolve_fixture_error_type(fixtures);
        sig.output = syn::parse_quote! { -> ::std::result::Result<(), #error_ty> };
        return_kind = ScenarioReturnKind::ResultUnit;
    }

    return_kind
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

    let alias = runtime_compatibility_alias(ctx.runtime);
    let resolved_harness = resolve_harness_path(ctx.harness, alias);
    let harness_ref = resolved_harness.as_ref();
    let effective_runtime = resolve_effective_runtime(ctx.runtime, alias, ctx.harness);

    let is_async = effective_runtime.is_async();
    let mut sig = build_test_signature(&fn_ident, &fixture_params, &example_params, is_async);

    let (_args, fixture_setup) = match extract_function_fixtures(&mut sig) {
        Ok(result) => result,
        Err(err) => return err.to_compile_error(),
    };

    let return_kind =
        resolve_scenario_return_kind(&mut sig, fixture_setup.has_result_fixtures, ctx.fixtures);

    let feature_path = ctx.manifest_dir.join(ctx.rel_path).display().to_string();
    let vis = syn::Visibility::Inherited;
    let block: syn::Block = if return_kind.is_fallible() {
        syn::parse_quote!({ Ok(()) })
    } else {
        syn::parse_quote!({})
    };

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
        runtime: effective_runtime,
        return_kind,
        harness: harness_ref,
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
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code uses infallible expects and indexed access for clarity"
)]
mod tests;
