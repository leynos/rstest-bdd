//! Implementation of the `#[scenario]` macro.
//! Binds tests to Gherkin scenarios and validates steps when compile-time flags enable it.

use cfg_if::cfg_if;
use proc_macro::TokenStream;
use std::path::{Path, PathBuf};

use crate::codegen::scenario::{ScenarioConfig, generate_scenario_code};
use crate::parsing::feature::{ScenarioData, extract_scenario_steps, parse_and_load_feature};
use crate::utils::fixtures::extract_function_fixtures;
use crate::validation::parameters::process_scenario_outline_examples;

use syn::{
    LitInt, LitStr,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::Comma,
};

struct ScenarioArgs {
    path: LitStr,
    index: Option<usize>,
}

enum ScenarioArg {
    Path(LitStr),
    Index(usize),
}

impl Parse for ScenarioArg {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(LitStr) {
            let lit: LitStr = input.parse()?;
            Ok(Self::Path(lit))
        } else {
            let ident: syn::Ident = input.parse()?;
            input.parse::<syn::token::Eq>()?;
            if ident == "path" {
                Ok(Self::Path(input.parse()?))
            } else if ident == "index" {
                let li: LitInt = input.parse()?;
                Ok(Self::Index(li.base10_parse()?))
            } else {
                Err(input.error("expected `path` or `index`"))
            }
        }
    }
}

impl Parse for ScenarioArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let args = Punctuated::<ScenarioArg, Comma>::parse_terminated(input)?;
        let mut path = None;
        let mut index = None;

        for arg in args {
            match arg {
                ScenarioArg::Path(lit) => {
                    if path.is_some() {
                        return Err(input.error("duplicate `path` argument"));
                    }
                    path = Some(lit);
                }
                ScenarioArg::Index(i) => {
                    if index.is_some() {
                        return Err(input.error("duplicate `index` argument"));
                    }
                    index = Some(i);
                }
            }
        }

        let path = path.ok_or_else(|| input.error("`path` is required"))?;

        Ok(Self { path, index })
    }
}

pub(crate) fn scenario(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(attr as ScenarioArgs);
    let item_fn = syn::parse_macro_input!(item as syn::ItemFn);
    match try_scenario(args, item_fn) {
        Ok(tokens) => tokens,
        Err(err) => err,
    }
}

fn try_scenario(
    ScenarioArgs { path, index }: ScenarioArgs,
    mut item_fn: syn::ItemFn,
) -> std::result::Result<TokenStream, TokenStream> {
    let path = PathBuf::from(path.value());
    let attrs = &item_fn.attrs;
    let vis = &item_fn.vis;
    let sig = &mut item_fn.sig;
    let block = &item_fn.block;

    let feature = parse_and_load_feature(&path).map_err(proc_macro::TokenStream::from)?;
    let feature_path_str = canonical_feature_path(&path);
    let ScenarioData {
        name: scenario_name,
        steps,
        examples,
    } = extract_scenario_steps(&feature, index).map_err(proc_macro::TokenStream::from)?;

    if let Some(err) = validate_steps_compile_time(&steps) {
        return Err(err);
    }

    process_scenario_outline_examples(sig, examples.as_ref())
        .map_err(proc_macro::TokenStream::from)?;

    let (_args, ctx_inserts) = extract_function_fixtures(sig);

    Ok(generate_scenario_code(
        ScenarioConfig {
            attrs,
            vis,
            sig,
            block,
            feature_path: feature_path_str,
            scenario_name,
            steps,
            examples,
        },
        ctx_inserts,
    ))
}

/// Canonicalise the feature path for stable diagnostics.
///
/// ```rust,ignore
/// # use std::path::{Path, PathBuf};
/// # fn demo() {
/// let path = PathBuf::from("features/example.feature");
/// let _ = canonical_feature_path(&path);
/// # }
/// ```
fn canonical_feature_path(path: &Path) -> String {
    std::env::var("CARGO_MANIFEST_DIR")
        .ok()
        .map(PathBuf::from)
        .map(|d| d.join(path))
        .and_then(|p| std::fs::canonicalize(&p).ok())
        .unwrap_or_else(|| PathBuf::from(path))
        .display()
        .to_string()
}

/// Validate registered steps when compile-time validation is enabled.
///
/// ```rust,ignore,ignore
/// let steps = Vec::new();
/// let _ = validate_steps_compile_time(&steps);
/// ```
fn validate_steps_compile_time(
    steps: &[crate::parsing::feature::ParsedStep],
) -> Option<TokenStream> {
    // When both features are enabled, strict mode wins.
    cfg_if! {
        if #[cfg(feature = "strict-compile-time-validation")] {
            crate::validation::steps::validate_steps_exist(steps, true)
                .err()
                .map(|e| e.into_compile_error().into())
        } else if #[cfg(feature = "compile-time-validation")] {
            crate::validation::steps::validate_steps_exist(steps, false)
                .err()
                .map(|e| e.into_compile_error().into())
        } else {
            None
        }
    }
}
#[cfg(test)]
mod tests {
    use super::canonical_feature_path;
    use rstest::rstest;
    use std::env;
    use std::path::{Path, PathBuf};

    #[rstest]
    fn canonicalises_with_manifest_dir() {
        let manifest = match env::var("CARGO_MANIFEST_DIR") {
            Ok(m) => PathBuf::from(m),
            Err(e) => panic!("manifest dir: {e}"),
        };
        let path = Path::new("Cargo.toml");
        let expected = match manifest.join(path).canonicalize() {
            Ok(p) => p.display().to_string(),
            Err(e) => panic!("canonical path: {e}"),
        };
        assert_eq!(canonical_feature_path(path), expected);
    }

    #[rstest]
    fn falls_back_on_missing_path() {
        let path = Path::new("does-not-exist.feature");
        assert_eq!(canonical_feature_path(path), path.display().to_string());
    }
}
