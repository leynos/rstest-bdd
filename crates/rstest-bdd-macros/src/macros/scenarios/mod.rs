//! Entry-point for the `scenarios!` macro.
//!
//! The module is split into focused helpers: `macro_args` parses the macro
//! input, `feature_discovery` walks the filesystem to enumerate `.feature`
//! files, `path_resolution` canonicalises paths so diagnostics remain stable
//! across builds, and `test_generation` creates rstest-backed test functions.
//! This file stitches those pieces together, applying any compile-time tag
//! filters and generating the rstest-backed test functions.

mod feature_discovery;
mod macro_args;
mod path_resolution;
mod test_generation;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::parsing::feature::{extract_scenario_steps, parse_and_load_feature};
use crate::parsing::tags::TagExpression;
use crate::utils::errors::{error_to_tokens, normalized_dir_read_error};
use crate::utils::ident::sanitize_ident;

use self::feature_discovery::collect_feature_files;
use self::macro_args::{FixtureSpec, RuntimeMode, ScenariosArgs};
use self::test_generation::{ScenarioTestContext, generate_scenario_test};

pub(crate) use self::macro_args::RuntimeMode as ScenariosRuntimeMode;

struct TagFilter {
    expr: TagExpression,
    span: Span,
    raw: String,
}

/// Context for processing feature files, bundling configuration
/// that remains constant across multiple feature file operations.
struct FeatureProcessingContext<'a> {
    manifest_dir: &'a Path,
    tag_filter: Option<&'a TagExpression>,
    fixtures: &'a [FixtureSpec],
    runtime: RuntimeMode,
}

#[expect(
    clippy::single_match_else,
    clippy::option_if_let_else,
    reason = "explicit match clarifies control flow"
)]
fn resolve_manifest_directory() -> Result<PathBuf, TokenStream> {
    match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(v) => Ok(PathBuf::from(v)),
        Err(_) => {
            let err = syn::Error::new(
                Span::call_site(),
                "CARGO_MANIFEST_DIR is not set. This macro must run within Cargo.",
            );
            Err(error_to_tokens(&err).into())
        }
    }
}

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
        manifest_dir: ctx.manifest_dir,
        rel_path: &rel_path,
        tag_filter: ctx.tag_filter,
        fixtures: ctx.fixtures,
        runtime: ctx.runtime,
    };

    process_scenarios(&feature, &test_ctx, used_names)
}

fn generate_tests_from_features(
    feature_paths: Vec<PathBuf>,
    manifest_dir: &Path,
    tag_filter: Option<&TagExpression>,
    fixtures: &[FixtureSpec],
    runtime: RuntimeMode,
) -> (Vec<TokenStream2>, Vec<TokenStream2>) {
    let mut used_names = HashSet::new();
    let mut tests = Vec::new();
    let mut errors = Vec::new();

    let ctx = FeatureProcessingContext {
        manifest_dir,
        tag_filter,
        fixtures,
        runtime,
    };

    for abs_path in feature_paths {
        let (mut t, mut errs) = process_feature_file(abs_path.as_path(), &ctx, &mut used_names);
        tests.append(&mut t);
        errors.append(&mut errs);
    }

    (tests, errors)
}

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

fn check_empty_results(
    tests: &[TokenStream2],
    errors: &mut Vec<TokenStream2>,
    tag_filter: Option<&TagFilter>,
) {
    if tests.is_empty() && errors.is_empty() {
        if let Some(filter) = tag_filter {
            let err = syn::Error::new(
                filter.span,
                format!("no scenarios matched tag expression `{}`", filter.raw),
            );
            errors.push(error_to_tokens(&err));
        }
    }
}

pub(crate) fn scenarios(input: TokenStream) -> TokenStream {
    let ScenariosArgs {
        dir: dir_lit,
        tag_filter: tag_lit,
        fixtures,
        runtime,
    } = syn::parse_macro_input!(input as ScenariosArgs);
    let dir = PathBuf::from(dir_lit.value());

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

    let (tests, mut errors) = generate_tests_from_features(
        feature_paths,
        &manifest_dir,
        tag_filter.as_ref().map(|f| &f.expr),
        &fixtures,
        runtime,
    );

    check_empty_results(&tests, &mut errors, tag_filter.as_ref());

    let module_ident = {
        let base = dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("scenarios");
        format_ident!("{}_scenarios", sanitize_ident(base))
    };
    let module_doc = format!("Scenarios auto-generated from `{}`.", dir_lit.value());

    TokenStream::from(quote! {
        #[doc = #module_doc]
        mod #module_ident {
            use super::*;
            #(#tests)*
            #(#errors)*
        }
    })
}

#[cfg(unix)]
#[cfg(test)]
mod tests {
    use super::feature_discovery::collect_feature_files;
    use std::fs;
    use std::io;
    use std::os::unix::fs::symlink;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn collects_symlinked_feature_files_without_following_directory_loops() -> io::Result<()> {
        let temp = tempdir()?;
        let features_root = temp.path().join("features");
        fs::create_dir_all(features_root.join("nested"))?;

        let feature_path = features_root.join("nested/example.feature");
        fs::write(&feature_path, "Feature: Example\n")?;

        let symlink_path = features_root.join("symlink.feature");
        symlink(&feature_path, &symlink_path)?;

        let relative_symlink_path = features_root.join("relative_link.feature");
        symlink(Path::new("nested/example.feature"), &relative_symlink_path)?;

        let loop_dir = features_root.join("loop");
        symlink(&features_root, &loop_dir)?;

        let files = collect_feature_files(features_root.as_path())?;

        let mut expected = vec![feature_path, symlink_path, relative_symlink_path];
        expected.sort();
        assert_eq!(files, expected);

        Ok(())
    }
}

#[cfg(not(unix))]
#[cfg(test)]
mod tests {
    #[test]
    fn collects_symlinked_feature_files_without_following_directory_loops() {
        assert!(cfg!(not(unix)));
    }
}
