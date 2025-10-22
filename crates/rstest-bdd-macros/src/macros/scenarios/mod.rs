//! Implementation of the `scenarios!` macro.

mod feature_discovery;
mod macro_args;
mod path_resolution;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::codegen::scenario::{ScenarioConfig, generate_scenario_code};
use crate::parsing::feature::{ScenarioData, extract_scenario_steps, parse_and_load_feature};
use crate::parsing::tags::TagExpression;
use crate::utils::errors::{error_to_tokens, normalized_dir_read_error};
use crate::utils::fixtures::extract_function_fixtures;
use crate::utils::ident::sanitize_ident;

use self::feature_discovery::collect_feature_files;
use self::macro_args::ScenariosArgs;

struct ScenarioTestContext<'a> {
    feature_stem: &'a str,
    manifest_dir: &'a Path,
    rel_path: &'a Path,
}

fn dedupe_name(base: &str, used: &mut HashSet<String>) -> String {
    let mut name = base.to_string();
    let mut counter = 1usize;
    while used.contains(&name) {
        name = format!("{base}_{counter}");
        counter += 1;
    }
    used.insert(name.clone());
    name
}

struct TagFilter {
    expr: TagExpression,
    span: Span,
    raw: String,
}

fn generate_scenario_test(
    ctx: &ScenarioTestContext<'_>,
    used_names: &mut HashSet<String>,
    data: ScenarioData,
) -> TokenStream2 {
    let ScenarioData {
        name,
        steps,
        examples,
        ..
    } = data;
    let base_name = format!("{}_{}", ctx.feature_stem, sanitize_ident(&name));
    let fn_name = dedupe_name(&base_name, used_names);
    let fn_ident = format_ident!("{}", fn_name);

    let attrs: Vec<syn::Attribute> = Vec::new();
    let vis = syn::Visibility::Inherited;
    let mut sig: syn::Signature = examples.as_ref().map_or_else(
        || syn::parse_quote! { fn #fn_ident() },
        |ex| {
            let params = ex.headers.iter().map(|h| {
                let param_ident = format_ident!("{}", sanitize_ident(h));
                quote! { #[case] #param_ident: &'static str }
            });
            syn::parse_quote! { fn #fn_ident( #(#params),* ) }
        },
    );
    let Ok((_args, ctx_inserts)) = extract_function_fixtures(&mut sig) else {
        unreachable!("generated scenario signature must bind fixtures");
    };
    let block: syn::Block = syn::parse_quote!({});

    let feature_path = ctx.manifest_dir.join(ctx.rel_path).display().to_string();

    let config = ScenarioConfig {
        attrs: &attrs,
        vis: &vis,
        sig: &sig,
        block: &block,
        feature_path,
        scenario_name: name,
        steps,
        examples,
    };
    TokenStream2::from(generate_scenario_code(config, ctx_inserts.into_iter()))
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
    tag_filter: Option<&TagExpression>,
) -> (Vec<TokenStream2>, Vec<TokenStream2>) {
    let mut tests = Vec::new();
    let mut errors = Vec::new();

    for (idx, _) in feature.scenarios.iter().enumerate() {
        match extract_scenario_steps(feature, Some(idx)) {
            Ok(mut data) => {
                if tag_filter.is_none_or(|filter| data.filter_by_tags(filter)) {
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
    manifest_dir: &Path,
    used_names: &mut HashSet<String>,
    tag_filter: Option<&TagExpression>,
) -> (Vec<TokenStream2>, Vec<TokenStream2>) {
    let rel_path = abs_path
        .strip_prefix(manifest_dir)
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
    let ctx = ScenarioTestContext {
        feature_stem: &feature_stem,
        manifest_dir,
        rel_path: &rel_path,
    };

    process_scenarios(&feature, &ctx, used_names, tag_filter)
}

fn generate_tests_from_features(
    feature_paths: Vec<PathBuf>,
    manifest_dir: &Path,
    tag_filter: Option<&TagExpression>,
) -> (Vec<TokenStream2>, Vec<TokenStream2>) {
    let mut used_names = HashSet::new();
    let mut tests = Vec::new();
    let mut errors = Vec::new();

    for abs_path in feature_paths {
        let (mut t, mut errs) = process_feature_file(
            abs_path.as_path(),
            manifest_dir,
            &mut used_names,
            tag_filter,
        );
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

    #[cfg(unix)]
    mod unix {
        use super::super::feature_discovery::collect_feature_files;
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
    #[test]
    fn collects_symlinked_feature_files_without_following_directory_loops() {
        assert!(cfg!(not(unix)));
    }
}
