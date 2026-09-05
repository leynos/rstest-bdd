//! Entry-point for the `scenarios!` macro.
//!
//! The module is split into focused helpers: `macro_args` parses the macro
//! input, `feature_discovery` walks the filesystem to enumerate `.feature`
//! files, `path_resolution` canonicalizes paths so diagnostics remain stable
//! across builds, and `test_generation` creates rstest-backed test functions.
//! This file stitches those pieces together, applying any compile-time tag
//! filters and generating the rstest-backed test functions.

mod feature_discovery;
mod macro_args;
mod path_resolution;
mod test_generation;

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};

pub(crate) use self::macro_args::{
    RuntimeMode as ScenariosRuntimeMode,
    TestAttributeHint as ScenariosTestAttributeHint,
};
use self::{
    feature_discovery::collect_feature_files,
    macro_args::{FixtureSpec, RuntimeMode, ScenariosArgs, runtime_compatibility_alias},
    test_generation::{ScenarioTestContext, generate_scenario_test, resolve_harness_path},
};
use crate::{
    parsing::{
        feature::{extract_scenario_steps, parse_and_load_feature},
        tags::TagExpression,
    },
    utils::{
        errors::{error_to_tokens, normalized_dir_read_error},
        ident::sanitize_ident,
        warnings::emit_warning,
    },
};

/// Internal data used by the macros implementation.
struct TagFilter {
    /// Stores the internal `expr` value.
    expr: TagExpression,
    /// Stores the internal `span` value.
    span: Span,
    /// Stores the internal `raw` value.
    raw: String,
}

/// Context for processing feature files, bundling configuration
/// that remains constant across multiple feature file operations.
struct FeatureProcessingContext<'a> {
    /// Stores the internal `manifest_dir` value.
    manifest_dir: &'a Path,
    /// Stores the internal `tag_filter` value.
    tag_filter: Option<&'a TagExpression>,
    /// Stores the internal `fixtures` value.
    fixtures: &'a [FixtureSpec],
    /// Stores the internal `runtime` value.
    runtime: RuntimeMode,
    /// Stores the internal `harness` value.
    harness: Option<&'a syn::Path>,
    /// Stores the internal `attributes` value.
    attributes: Option<&'a syn::Path>,
    /// Harness path actually generated against, after applying any runtime
    /// compatibility alias. Resolved once so every scenario agrees with the
    /// single diagnostic emitted at the expansion boundary.
    effective_harness: Option<&'a syn::Path>,
    /// Adapter API paths resolved once for the whole `scenarios!` expansion.
    resolutions: &'a crate::codegen::SharedAdapterResolutions,
}

/// Provides the internal `resolve_manifest_directory` operation.
fn resolve_manifest_directory() -> Result<PathBuf, TokenStream> {
    std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .map_err(|_| {
            let err = syn::Error::new(
                Span::call_site(),
                "CARGO_MANIFEST_DIR is not set. This macro must run within Cargo.",
            );
            error_to_tokens(&err).into()
        })
}

/// Provides the internal `process_scenarios` operation.
fn process_scenarios(
    feature: &gherkin::Feature,
    ctx: &ScenarioTestContext<'_>,
    used_names: &mut HashSet<String>,
) -> (Vec<TokenStream2>, Vec<TokenStream2>) {
    let mut tests = Vec::new();
    let mut errors = Vec::new();

    for idx in 0..feature.scenarios.len() {
        match extract_scenario_steps(feature, Some(idx)) {
            Ok(mut data) => {
                if ctx
                    .tag_filter
                    .is_none_or(|filter| data.filter_by_tags(filter))
                {
                    tests.push(generate_scenario_test(ctx, used_names, data));
                }
            }
            Err(err) => errors.push(err),
        }
    }

    (tests, errors)
}

/// Provides the internal `process_feature_file` operation.
fn process_feature_file(
    abs_path: &Path,
    ctx: &FeatureProcessingContext<'_>,
    used_names: &mut HashSet<String>,
) -> (Vec<TokenStream2>, Vec<TokenStream2>) {
    let rel_path = abs_path
        .strip_prefix(ctx.manifest_dir)
        .map_or_else(|_| abs_path.to_path_buf(), Path::to_path_buf);

    let feature = match parse_and_load_feature(rel_path.as_path()) {
        Ok(feature) => feature,
        Err(err) => return (Vec::new(), vec![err]),
    };

    let feature_stem = sanitize_ident(
        rel_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("feature"),
    );
    let test_ctx = ScenarioTestContext {
        feature_stem: &feature_stem,
        rel_path: &rel_path,
        tag_filter: ctx.tag_filter,
        fixtures: ctx.fixtures,
        runtime: ctx.runtime,
        harness: ctx.harness,
        attributes: ctx.attributes,
        effective_harness: ctx.effective_harness,
        resolutions: ctx.resolutions,
    };

    process_scenarios(&feature, &test_ctx, used_names)
}

/// Provides the internal `generate_tests_from_features` operation.
fn generate_tests_from_features(
    feature_paths: &[PathBuf],
    ctx: &FeatureProcessingContext<'_>,
) -> (Vec<TokenStream2>, Vec<TokenStream2>) {
    let mut used_names = HashSet::new();
    let mut tests = Vec::new();
    let mut errors = Vec::new();

    for abs_path in feature_paths {
        let (mut t, mut errs) = process_feature_file(abs_path.as_path(), ctx, &mut used_names);
        tests.append(&mut t);
        errors.append(&mut errs);
    }

    (tests, errors)
}

/// Provides the internal `parse_tag_filter` operation.
fn parse_tag_filter(tag_lit: Option<syn::LitStr>) -> Result<Option<TagFilter>, TokenStream> {
    tag_lit.map_or_else(
        || Ok(None),
        |lit| match TagExpression::parse(&lit.value()) {
            Ok(expr) => Ok(Some(TagFilter {
                expr,
                span: lit.span(),
                raw: lit.value(),
            })),
            Err(err) => {
                let syn_err = syn::Error::new(lit.span(), err.to_string());
                Err(error_to_tokens(&syn_err).into())
            }
        },
    )
}

/// Provides the internal `check_empty_results` operation.
fn check_empty_results(
    tests: &[TokenStream2],
    errors: &mut Vec<TokenStream2>,
    tag_filter: Option<&TagFilter>,
) {
    let is_nothing_emitted = tests.is_empty() && errors.is_empty();
    if is_nothing_emitted && let Some(filter) = tag_filter {
        let err = syn::Error::new(
            filter.span,
            format!("no scenarios matched tag expression `{}`", filter.raw),
        );
        errors.push(error_to_tokens(&err));
    }
}

/// Provides the internal `emit_runtime_deprecation_warning` operation.
fn emit_runtime_deprecation_warning(runtime: RuntimeMode, harness: Option<&syn::Path>) {
    if runtime != RuntimeMode::TokioCurrentThread {
        return;
    }
    if harness.is_some() {
        emit_warning(
            Span::call_site(),
            concat!(
                "the `runtime = \"tokio-current-thread\"` argument is ",
                "deprecated and redundant when an explicit `harness` is set; ",
                "remove the `runtime` argument"
            )
            .to_owned(),
            None,
        );
    } else {
        emit_warning(
            Span::call_site(),
            concat!(
                "the `runtime = \"tokio-current-thread\"` syntax is ",
                "deprecated; use ",
                "`harness = rstest_bdd_harness_tokio::TokioHarness` instead"
            )
            .to_owned(),
            None,
        );
    }
}

/// Inputs required to emit the items produced by a `scenarios!` expansion.
struct ScenariosModuleEmission<'a> {
    /// Parsed directory, retained to preserve the existing module-name derivation.
    dir: &'a Path,
    /// Directory literal that supplies the generated module's name, docs, and
    /// tracking-item span.
    dir_lit: &'a syn::LitStr,
    /// Every discovered feature file, including files excluded by a tag filter.
    feature_paths: &'a [PathBuf],
    /// Diagnostics emitted before generated tests within the module.
    fallback_diagnostics: TokenStream2,
    /// Generated scenario-test items.
    tests: Vec<TokenStream2>,
    /// Generated error items.
    errors: Vec<TokenStream2>,
}

/// Emits tracking items and the generated scenario module in source order.
fn emit_scenarios_module(emission: ScenariosModuleEmission<'_>) -> TokenStream {
    let ScenariosModuleEmission {
        dir,
        dir_lit,
        feature_paths,
        fallback_diagnostics,
        tests,
        errors,
    } = emission;

    // Emit one Cargo rebuild-dependency tracking item per **discovered**
    // file, as a sibling of the generated scenario module, independent of
    // whether any test is generated for that file. A `tags =` filter that
    // matches nothing in a particular file still parses it, and that file
    // must remain tracked — a body-scoped or per-test binding would leave it
    // untracked and reopen the foot-gun in exactly the macro that is the
    // harder case (Decision D0 / ExecPlan Milestone 3).
    let tracking_items = feature_paths
        .iter()
        .map(|path| crate::codegen::tracking::feature_tracking_item(path, dir_lit.span()))
        .collect::<Vec<_>>();

    let module_ident = {
        let base = dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("scenarios");
        format_ident!("{}_scenarios", sanitize_ident(base))
    };
    let module_doc = format!("Scenarios auto-generated from `{}`.", dir_lit.value());

    TokenStream::from(quote! {
        #(#tracking_items)*
        #[doc = #module_doc]
        mod #module_ident {
            use super::*;
            #fallback_diagnostics
            #(#tests)*
            #(#errors)*
        }
    })
}

/// Provides the internal `scenarios` operation.
pub(crate) fn scenarios(input: TokenStream) -> TokenStream {
    let ScenariosArgs {
        dir: dir_lit,
        tag_filter: tag_lit,
        fixtures,
        runtime,
        harness,
        attributes,
    } = syn::parse_macro_input!(input as ScenariosArgs);
    let dir = PathBuf::from(dir_lit.value());

    emit_runtime_deprecation_warning(runtime, harness.as_ref());

    let tag_filter = match parse_tag_filter(tag_lit) {
        Ok(filter) => filter,
        Err(err_tokens) => return err_tokens,
    };

    let manifest_dir = match resolve_manifest_directory() {
        Ok(dir) => dir,
        Err(err_tokens) => return err_tokens,
    };

    let search_dir = manifest_dir.join(&dir);
    let feature_paths = match collect_feature_files(&search_dir) {
        Ok(paths) => paths,
        Err(err) => {
            let msg = normalized_dir_read_error(&search_dir, &err);
            let err = syn::Error::new(Span::call_site(), msg);
            return error_to_tokens(&err).into();
        }
    };

    // Resolve the supplied adapter paths once for the whole expansion. Every
    // generated scenario reuses this decision, so a feature directory with many
    // scenarios reports one diagnostic per supplied path instead of one per
    // generated test.
    let effective_harness =
        resolve_harness_path(harness.as_ref(), runtime_compatibility_alias(runtime));
    let resolutions = crate::codegen::SharedAdapterResolutions::resolve(
        effective_harness.as_ref(),
        attributes.as_ref(),
    );
    let fallback_diagnostics = resolutions.emit_diagnostics();

    let ctx = FeatureProcessingContext {
        manifest_dir: &manifest_dir,
        tag_filter: tag_filter.as_ref().map(|f| &f.expr),
        fixtures: &fixtures,
        runtime,
        harness: harness.as_ref(),
        attributes: attributes.as_ref(),
        effective_harness: effective_harness.as_ref(),
        resolutions: &resolutions,
    };
    let (tests, mut errors) = generate_tests_from_features(&feature_paths, &ctx);

    check_empty_results(&tests, &mut errors, tag_filter.as_ref());

    emit_scenarios_module(ScenariosModuleEmission {
        dir: &dir,
        dir_lit: &dir_lit,
        feature_paths: &feature_paths,
        fallback_diagnostics,
        tests,
        errors,
    })
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
