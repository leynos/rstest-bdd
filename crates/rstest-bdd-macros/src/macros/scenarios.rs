//! Implementation of the `scenarios!` macro.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::codegen::scenario::{ScenarioConfig, generate_scenario_code};
use crate::parsing::feature::{extract_scenario_steps, parse_and_load_feature};
use crate::utils::errors::{error_to_tokens, normalized_dir_read_error};
use crate::utils::ident::sanitize_ident;
use gherkin::Feature;

/// Recursively collect all `.feature` files under `base`.
fn collect_feature_files(base: &Path) -> std::io::Result<Vec<PathBuf>> {
    use std::io;
    use walkdir::WalkDir;

    fn is_feature_file(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("feature"))
    }

    let mut files: Vec<PathBuf> = WalkDir::new(base)
        .follow_links(true)
        .into_iter()
        .filter_map(|entry| match entry {
            Ok(e) if e.file_type().is_file() && is_feature_file(e.path()) => {
                Some(Ok(e.into_path()))
            }
            Ok(_) => None,
            Err(e) => {
                let err_str = e.to_string();
                let io_err = e
                    .into_io_error()
                    .unwrap_or_else(|| io::Error::other(err_str));
                Some(Err(io_err))
            }
        })
        .collect::<Result<_, _>>()?;

    files.sort();
    Ok(files)
}

/// Generate the test for a single scenario within a feature.
/// Context for generating a scenario test.
struct ScenarioTestContext<'a> {
    feature: &'a Feature,
    scenario_idx: usize,
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

fn generate_scenario_test(
    ctx: &ScenarioTestContext<'_>,
    used_names: &mut HashSet<String>,
) -> Result<TokenStream2, TokenStream> {
    let data = extract_scenario_steps(ctx.feature, Some(ctx.scenario_idx))?;
    let base_name = format!("{}_{}", ctx.feature_stem, sanitize_ident(&data.name));
    let fn_name = dedupe_name(&base_name, used_names);
    let fn_ident = format_ident!("{}", fn_name);

    let attrs: Vec<syn::Attribute> = Vec::new();
    let vis = syn::Visibility::Inherited;
    let sig: syn::Signature = data.examples.as_ref().map_or_else(
        || syn::parse_quote! { fn #fn_ident() },
        |ex| {
            let params = ex.headers.iter().map(|h| {
                let param_ident = format_ident!("{}", sanitize_ident(h));
                quote! { #[case] #param_ident: &str }
            });
            syn::parse_quote! { fn #fn_ident( #(#params),* ) }
        },
    );
    let block: syn::Block = syn::parse_quote!({});

    let feature_path = ctx.manifest_dir.join(ctx.rel_path).display().to_string();

    let config = ScenarioConfig {
        attrs: &attrs,
        vis: &vis,
        sig: &sig,
        block: &block,
        feature_path,
        scenario_name: data.name,
        steps: data.steps,
        examples: data.examples,
    };
    Ok(TokenStream2::from(generate_scenario_code(
        config,
        std::iter::empty(),
    )))
}

/// Resolve the Cargo manifest directory or return a compile error.
///
/// # Examples
///
/// ```rust,ignore
/// std::env::set_var("CARGO_MANIFEST_DIR", "/tmp");
/// let path =
///     resolve_manifest_directory().expect("CARGO_MANIFEST_DIR is set");
/// assert_eq!(path, std::path::PathBuf::from("/tmp"));
/// ```
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

/// Generate the test code for every scenario inside a single feature file.
///
/// Deduplicates test names using `used_names` and collects errors without
/// short-circuiting.
fn process_feature_file(
    abs_path: &Path,
    manifest_dir: &Path,
    used_names: &mut HashSet<String>,
) -> (Vec<TokenStream2>, Vec<TokenStream2>) {
    let rel_path = abs_path
        .strip_prefix(manifest_dir)
        .map_or_else(|_| abs_path.to_path_buf(), Path::to_path_buf);

    let mut tests = Vec::new();
    let mut errors = Vec::new();

    match parse_and_load_feature(rel_path.as_path()) {
        Ok(feature) => {
            let feature_stem = sanitize_ident(
                rel_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("feature"),
            );
            for (idx, _) in feature.scenarios.iter().enumerate() {
                let ctx = ScenarioTestContext {
                    feature: &feature,
                    scenario_idx: idx,
                    feature_stem: &feature_stem,
                    manifest_dir,
                    rel_path: &rel_path,
                };
                match generate_scenario_test(&ctx, used_names) {
                    Ok(ts) => tests.push(ts),
                    Err(err) => errors.push(TokenStream2::from(err)),
                }
            }
        }
        Err(err) => errors.push(err),
    }

    (tests, errors)
}

/// Generate tests for the provided feature paths, returning any errors.
fn generate_tests_from_features(
    feature_paths: Vec<PathBuf>,
    manifest_dir: &Path,
) -> (Vec<TokenStream2>, Vec<TokenStream2>) {
    let mut used_names = HashSet::new();
    let mut tests = Vec::new();
    let mut errors = Vec::new();
    for abs_path in feature_paths {
        let (mut t, mut errs) =
            process_feature_file(abs_path.as_path(), manifest_dir, &mut used_names);
        tests.append(&mut t);
        errors.append(&mut errs);
    }
    (tests, errors)
}

/// Generate test modules for all scenarios within a directory of feature files.
pub(crate) fn scenarios(input: TokenStream) -> TokenStream {
    let dir_lit = syn::parse_macro_input!(input as syn::LitStr);
    let dir = PathBuf::from(dir_lit.value());

    let manifest_dir = match resolve_manifest_directory() {
        Ok(dir) => dir,
        Err(err_tokens) => return err_tokens,
    };

    let search_dir = manifest_dir.join(&dir);
    let feature_paths_res = collect_feature_files(&search_dir);
    if let Err(err) = feature_paths_res {
        let msg = normalized_dir_read_error(&search_dir, &err);
        let err = syn::Error::new(Span::call_site(), msg);
        return error_to_tokens(&err).into();
    }
    let Ok(feature_paths) = feature_paths_res else {
        unreachable!("checked Err above");
    };

    let (tests, errors) = generate_tests_from_features(feature_paths, &manifest_dir);

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
}
