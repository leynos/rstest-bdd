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
        let args =
            syn::punctuated::Punctuated::<syn::Expr, syn::token::Comma>::parse_terminated(input)?;
        let mut path = None;
        let mut index = None;

        for expr in args {
            match expr {
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(lit),
                    ..
                }) if path.is_none() => {
                    path = Some(lit);
                }
                syn::Expr::Assign(assign) => {
                    let ident = if let syn::Expr::Path(p) = &*assign.left {
                        p.path.get_ident().cloned().ok_or_else(|| {
                            syn::Error::new_spanned(&assign.left, "expected identifier")
                        })?
                    } else {
                        return Err(syn::Error::new_spanned(&assign.left, "expected identifier"));
                    };
                    match ident.to_string().as_str() {
                        "path" => {
                            let syn::Expr::Lit(syn::ExprLit {
                                lit: syn::Lit::Str(lit),
                                ..
                            }) = *assign.right
                            else {
                                return Err(syn::Error::new_spanned(
                                    &assign.right,
                                    "expected string literal",
                                ));
                            };
                            path = Some(lit);
                        }
                        "index" => {
                            let syn::Expr::Lit(syn::ExprLit {
                                lit: syn::Lit::Int(lit),
                                ..
                            }) = *assign.right
                            else {
                                return Err(syn::Error::new_spanned(
                                    &assign.right,
                                    "expected integer literal",
                                ));
                            };
                            index = Some(lit.base10_parse()?);
                        }
                        _ => {
                            return Err(syn::Error::new_spanned(
                                &assign.left,
                                "expected `path` or `index`",
                            ));
                        }
                    }
                }
                other => {
                    return Err(syn::Error::new_spanned(other, "unexpected argument"));
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
