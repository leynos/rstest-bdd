//! Implementation of the `#[scenario]` macro.

use proc_macro::TokenStream;
use std::path::PathBuf;

use crate::codegen::scenario::generate_scenario_code;
use crate::parsing::feature::{ScenarioData, extract_scenario_steps, parse_and_load_feature};
use crate::utils::fixtures::extract_function_fixtures;
use crate::validation::parameters::process_scenario_outline_examples;

struct ScenarioArgs {
    path: syn::LitStr,
    index: Option<usize>,
}

impl syn::parse::Parse for ScenarioArgs {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(syn::LitStr) {
            Self::parse_bare_string(input)
        } else {
            Self::parse_named_args(input)
        }
    }
}

impl ScenarioArgs {
    fn parse_bare_string(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let path: syn::LitStr = input.parse()?;
        let mut index = None;

        if input.peek(syn::token::Comma) {
            input.parse::<syn::token::Comma>()?;
            let ident: syn::Ident = input.parse()?;
            if ident != "index" {
                return Err(input.error("expected `index`"));
            }
            input.parse::<syn::token::Eq>()?;
            let lit: syn::LitInt = input.parse()?;
            index = Some(lit.base10_parse()?);
        }

        if !input.is_empty() {
            return Err(input.error("unexpected tokens"));
        }

        Ok(Self { path, index })
    }

    fn parse_named_args(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let mut path = None;
        let mut index = None;

        while !input.is_empty() {
            let ident: syn::Ident = input.parse()?;
            input.parse::<syn::token::Eq>()?;
            if ident == "path" {
                let lit: syn::LitStr = input.parse()?;
                path = Some(lit);
            } else if ident == "index" {
                let lit: syn::LitInt = input.parse()?;
                index = Some(lit.base10_parse()?);
            } else {
                return Err(input.error("expected `path` or `index`"));
            }

            if input.peek(syn::token::Comma) {
                input.parse::<syn::token::Comma>()?;
            } else {
                break;
            }
        }

        let Some(path) = path else {
            return Err(input.error("`path` is required"));
        };

        if !input.is_empty() {
            return Err(input.error("unexpected tokens"));
        }

        Ok(Self { path, index })
    }
}

/// Bind a test to a scenario defined in a feature file.
pub(crate) fn scenario(attr: TokenStream, item: TokenStream) -> TokenStream {
    let ScenarioArgs { path, index } = syn::parse_macro_input!(attr as ScenarioArgs);
    let path = PathBuf::from(path.value());

    let mut item_fn = syn::parse_macro_input!(item as syn::ItemFn);
    let attrs = &item_fn.attrs;
    let vis = &item_fn.vis;
    let sig = &mut item_fn.sig;
    let block = &item_fn.block;

    let feature = match parse_and_load_feature(&path) {
        Ok(f) => f,
        Err(err) => return err,
    };
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| String::new());
    let feature_path_str = PathBuf::from(manifest_dir)
        .join(&path)
        .display()
        .to_string();

    let ScenarioData {
        name: scenario_name,
        steps,
        examples,
    } = match extract_scenario_steps(&feature, index) {
        Ok(res) => res,
        Err(err) => return err,
    };

    if let Err(err) = process_scenario_outline_examples(sig, examples.as_ref()) {
        return err;
    }

    let (_args, ctx_inserts) = extract_function_fixtures(sig);

    generate_scenario_code(
        attrs,
        vis,
        sig,
        block,
        feature_path_str,
        scenario_name,
        steps,
        examples,
        ctx_inserts,
    )
}
