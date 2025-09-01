//! Implementation of the `#[scenario]` macro.

use proc_macro::TokenStream;
use std::path::PathBuf;

use crate::codegen::scenario::{ScenarioConfig, generate_scenario_code};
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
    path: LitStr,
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

        let path = path.ok_or_else(|| input.error("`path` is required"))?;

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
        Err(err) => return err.into(),
    };
    let feature_path_str = std::env::var("CARGO_MANIFEST_DIR")
        .ok()
        .map(PathBuf::from)
        .map(|d| d.join(&path))
        .and_then(|p| std::fs::canonicalize(&p).ok())
        .unwrap_or_else(|| PathBuf::from(&path))
        .display()
        .to_string();

    let ScenarioData {
        name: scenario_name,
        steps,
        examples,
    } = match extract_scenario_steps(&feature, index) {
        Ok(res) => res,
        Err(err) => return err.into(),
    };

    #[cfg(feature = "strict-compile-time-validation")]
    if let Err(err) = crate::validation::steps::validate_steps_exist(&steps, true) {
        return err.into_compile_error().into();
    }
    #[cfg(all(
        feature = "compile-time-validation",
        not(feature = "strict-compile-time-validation")
    ))]
    if let Err(err) = crate::validation::steps::validate_steps_exist(&steps, false) {
        return err.into_compile_error().into();
    }

    if let Err(err) = process_scenario_outline_examples(sig, examples.as_ref()) {
        return err.into();
    }

    let (_args, ctx_inserts) = extract_function_fixtures(sig);

    generate_scenario_code(
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
    )
}
