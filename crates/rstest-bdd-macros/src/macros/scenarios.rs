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

/// Sanitize a string so it may be used as a Rust identifier.
///
/// Non-alphanumeric characters are replaced with underscores and the result is
/// lowercased. Identifiers starting with a digit gain a leading underscore.
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
    let mut files = Vec::new();
    for entry in fs::read_dir(base)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_feature_files(&path)?);
        } else if path.extension().is_some_and(|e| e == "feature") {
            files.push(path);
        }
    }
    Ok(files)
}

/// Generate test modules for all scenarios within a directory of feature files.
pub(crate) fn scenarios(input: TokenStream) -> TokenStream {
    let dir_lit = syn::parse_macro_input!(input as syn::LitStr);
    let dir = PathBuf::from(dir_lit.value());

    let manifest_dir = if let Ok(v) = std::env::var("CARGO_MANIFEST_DIR") {
        PathBuf::from(v)
    } else {
        let err = syn::Error::new(
            Span::call_site(),
            "CARGO_MANIFEST_DIR is not set. This macro must run within Cargo.",
        );
        return error_to_tokens(&err);
    };

    let search_dir = manifest_dir.join(&dir);
    let feature_paths = match collect_feature_files(&search_dir) {
        Ok(v) => v,
        Err(err) => {
            let msg = format!("failed to read directory: {err}");
            let err = syn::Error::new(Span::call_site(), msg);
            return error_to_tokens(&err);
        }
    };

    let mut tests = Vec::new();
    let mut used_names = HashSet::new();

    for abs_path in feature_paths {
        let rel_path = abs_path
            .strip_prefix(&manifest_dir)
            .map_or_else(|_| abs_path.clone(), Path::to_path_buf);

        let feature = match parse_and_load_feature(rel_path.as_path()) {
            Ok(f) => f,
            Err(err) => return err,
        };

        let feature_stem = sanitize_ident(
            rel_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("feature"),
        );

        for (idx, _) in feature.scenarios.iter().enumerate() {
            let data = match extract_scenario_steps(&feature, Some(idx)) {
                Ok(d) => d,
                Err(err) => return err,
            };

            let base_name = format!("{}_{}", feature_stem, sanitize_ident(&data.name));
            let mut fn_name = base_name.clone();
            let mut counter = 1usize;
            while used_names.contains(&fn_name) {
                fn_name = format!("{base_name}_{counter}");
                counter += 1;
            }
            used_names.insert(fn_name.clone());
            let ident = format_ident!("{}", fn_name);

            let attrs: Vec<syn::Attribute> = Vec::new();
            let vis = syn::Visibility::Inherited;
            let sig: syn::Signature = syn::parse_quote! { fn #ident() };
            let block: syn::Block = syn::parse_quote!({});

            let feature_path = manifest_dir.join(&rel_path).display().to_string();

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
            let tokens = generate_scenario_code(config, std::iter::empty());
            tests.push(TokenStream2::from(tokens));
        }
    }

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
        }
    })
}

#[cfg(test)]
mod tests {
    use super::sanitize_ident;

    #[test]
    fn sanitises_invalid_identifiers() {
        assert_eq!(sanitize_ident("Hello world!"), "hello_world_");
    }
}
