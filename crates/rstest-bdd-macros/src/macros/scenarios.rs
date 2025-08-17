//! Implementation of the `scenarios!` macro.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::codegen::scenario::{ScenarioConfig, generate_scenario_code};
use crate::parsing::feature::{extract_scenario_steps, parse_and_load_feature};
use crate::utils::errors::error_to_tokens;
use gherkin::Feature;

/// Sanitize a string so it may be used as a Rust identifier.
///
/// Only ASCII alphanumeric characters are preserved; all other characters
/// (including Unicode) are replaced with underscores. The result is lowercased.
/// Identifiers starting with a digit gain a leading underscore.
///
/// Note: Unicode characters are not supported and will be replaced with
/// underscores.
/// TODO: Consider supporting Unicode normalisation in the future.
fn sanitize_ident(input: &str) -> String {
    let mut ident = String::new();
    for c in input.chars() {
        if c.is_ascii_alphanumeric() {
            ident.push(c.to_ascii_lowercase());
        } else {
            ident.push('_');
        }
    }
    if ident.is_empty() || ident.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        ident.insert(0, '_');
    }
    ident
}

/// Recursively collect all `.feature` files under `base`.
fn collect_feature_files(base: &Path) -> std::io::Result<Vec<PathBuf>> {
    fn is_feature_file(path: &Path) -> bool {
        path.extension().is_some_and(|e| e == "feature")
    }

    let mut files = Vec::new();
    for entry in fs::read_dir(base)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        let ft = metadata.file_type();

        if ft.is_symlink() {
            if fs::metadata(&path)
                .map(|t| t.file_type().is_file() && is_feature_file(&path))
                .unwrap_or(false)
            {
                files.push(path);
            }
            continue;
        }
        if ft.is_dir() {
            files.extend(collect_feature_files(&path)?);
            continue;
        }
        if ft.is_file() && is_feature_file(&path) {
            files.push(path);
        }
    }
    Ok(files)
}

/// Generate the test for a single scenario within a feature.
fn generate_scenario_test(
    feature: &Feature,
    scenario_idx: usize,
    feature_stem: &str,
    manifest_dir: &Path,
    rel_path: &Path,
    used_names: &mut HashSet<String>,
) -> Result<TokenStream2, TokenStream> {
    let data = extract_scenario_steps(feature, Some(scenario_idx))?;
    let base_name = format!("{}_{}", feature_stem, sanitize_ident(&data.name));
    let mut fn_name = base_name.clone();
    let mut counter = 1usize;
    while used_names.contains(&fn_name) {
        fn_name = format!("{base_name}_{counter}");
        counter += 1;
    }
    used_names.insert(fn_name.clone());
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

    let feature_path = manifest_dir.join(rel_path).display().to_string();

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
/// let path = resolve_manifest_directory().unwrap();
/// assert_eq!(path, std::path::PathBuf::from("/tmp"));
/// ```
fn resolve_manifest_directory() -> Result<PathBuf, TokenStream> {
    option_env!("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| {
            let err = syn::Error::new(
                Span::call_site(),
                "CARGO_MANIFEST_DIR is not set. This macro must run within Cargo.",
            );
            error_to_tokens(&err)
        })
}

/// Generate the test code for every scenario inside a single feature file.
///
/// Deduplicates test names using `used_names`.
///
/// # Examples
///
/// ```rust,ignore
/// # use std::collections::HashSet;
/// let mut used = HashSet::new();
/// let tests = process_feature_file(
///     std::path::Path::new("alpha.feature"),
///     std::path::Path::new("/tmp"),
///     &mut used,
/// )
/// .unwrap();
/// assert!(!tests.is_empty());
/// ```
fn process_feature_file(
    abs_path: &Path,
    manifest_dir: &Path,
    used_names: &mut HashSet<String>,
) -> Result<Vec<TokenStream2>, TokenStream> {
    let rel_path = abs_path
        .strip_prefix(manifest_dir)
        .map_or_else(|_| abs_path.to_path_buf(), Path::to_path_buf);

    let feature = parse_and_load_feature(rel_path.as_path())?;
    let feature_stem = sanitize_ident(
        rel_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("feature"),
    );

    let mut tests = Vec::new();
    let mut errors: Vec<TokenStream2> = Vec::new();
    for (idx, _) in feature.scenarios.iter().enumerate() {
        match generate_scenario_test(
            &feature,
            idx,
            &feature_stem,
            manifest_dir,
            &rel_path,
            used_names,
        ) {
            Ok(ts) => tests.push(ts),
            Err(err) => errors.push(TokenStream2::from(err)),
        }
    }

    if errors.is_empty() {
        Ok(tests)
    } else {
        Err(TokenStream::from(quote! { #(#errors)* }))
    }
}

/// Generate tests for the provided feature paths.
///
/// # Examples
///
/// ```rust,ignore
/// let (tests, errors) = generate_tests_from_features(
///     vec![std::path::PathBuf::from("alpha.feature")],
///     std::path::Path::new("/tmp"),
/// );
/// assert!(!tests.is_empty());
/// assert!(errors.is_empty());
/// ```
fn generate_tests_from_features(
    feature_paths: Vec<PathBuf>,
    manifest_dir: &Path,
) -> (Vec<TokenStream2>, Vec<TokenStream2>) {
    let mut used_names = HashSet::new();
    let mut tests = Vec::new();
    let mut errors: Vec<TokenStream2> = Vec::new();
    for abs_path in feature_paths {
        match process_feature_file(abs_path.as_path(), manifest_dir, &mut used_names) {
            Ok(mut t) => tests.append(&mut t),
            Err(err) => errors.push(TokenStream2::from(err)),
        }
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
        let msg = format!("failed to read directory `{}`: {err}", search_dir.display());
        let err = syn::Error::new(Span::call_site(), msg);
        return error_to_tokens(&err);
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
    use super::sanitize_ident;

    #[test]
    fn sanitizes_invalid_identifiers() {
        assert_eq!(sanitize_ident("Hello world!"), "hello_world_");
    }
}
