//! Implementation of the `#[scenario]` macro.

use proc_macro::TokenStream;
use proc_macro2::Span;
use std::path::PathBuf;

use crate::codegen::scenario::generate_scenario_code;
use crate::parsing::feature::{ScenarioData, extract_scenario_steps, parse_and_load_feature};
use crate::utils::fixtures::extract_function_fixtures;
use crate::validation::parameters::process_scenario_outline_examples;

use syn::{
    LitInt, LitStr, Result,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::Comma,
};

struct ScenarioArgs {
    path: Option<LitStr>,
    index: Option<usize>,
}

enum ScenarioArg {
    Path(LitStr),
    Index(usize),
}

impl Parse for ScenarioArg {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
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
    fn parse(input: ParseStream<'_>) -> Result<Self> {
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

        if path.is_none() && index.is_none() {
            return Err(input.error("at least one of `path` or `index` argument must be provided"));
        }

        Ok(Self { path, index })
    }
}

/// Bind a test to a scenario defined in a feature file.
pub(crate) fn scenario(attr: TokenStream, item: TokenStream) -> TokenStream {
    let ScenarioArgs { path, index } = syn::parse_macro_input!(attr as ScenarioArgs);
    let path = match path {
        Some(lit) => PathBuf::from(lit.value()),
        None => {
            return syn::Error::new(Span::call_site(), "`path` is required")
                .into_compile_error()
                .into();
        }
    };

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
