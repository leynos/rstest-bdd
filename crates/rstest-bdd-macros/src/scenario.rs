//! Scenario processing and test generation.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::path::PathBuf;

use gherkin::{Feature, StepType};
use syn::parse::{Parse, ParseStream};
use syn::token::{Comma, Eq};
use syn::{
    Attribute, Block, FnArg, ItemFn, LitInt, LitStr, Pat, PatType, Signature, Visibility,
    parse_macro_input,
};

use crate::feature::{ExampleTable, parse_and_load_feature};

pub(crate) struct ScenarioArgs {
    path: LitStr,
    index: Option<usize>,
}

impl Parse for ScenarioArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(LitStr) {
            Self::parse_bare_string(input)
        } else {
            Self::parse_named_args(input)
        }
    }
}

impl ScenarioArgs {
    fn parse_bare_string(input: ParseStream<'_>) -> syn::Result<Self> {
        let path: LitStr = input.parse()?;
        let mut index = None;

        if input.peek(Comma) {
            input.parse::<Comma>()?;
            let ident: syn::Ident = input.parse()?;
            if ident != "index" {
                return Err(input.error("expected `index`"));
            }
            input.parse::<Eq>()?;
            let lit: LitInt = input.parse()?;
            index = Some(lit.base10_parse()?);
        }

        if !input.is_empty() {
            return Err(input.error("unexpected tokens"));
        }

        Ok(Self { path, index })
    }

    fn parse_named_args(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut path = None;
        let mut index = None;

        while !input.is_empty() {
            let ident: syn::Ident = input.parse()?;
            input.parse::<Eq>()?;
            if ident == "path" {
                let lit: LitStr = input.parse()?;
                path = Some(lit);
            } else if ident == "index" {
                let lit: LitInt = input.parse()?;
                index = Some(lit.base10_parse()?);
            } else {
                return Err(input.error("expected `path` or `index`"));
            }

            if input.peek(Comma) {
                input.parse::<Comma>()?;
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

#[derive(Clone)]
pub(crate) struct ScenarioData {
    name: String,
    steps: Vec<(rstest_bdd::StepKeyword, String)>,
    examples: Option<ExampleTable>,
}

fn extract_examples(scenario: &gherkin::Scenario) -> Result<Option<ExampleTable>, TokenStream> {
    if !scenario.keyword.contains("Outline") && scenario.examples.is_empty() {
        return Ok(None);
    }

    let Some(first) = scenario.examples.first() else {
        let err = syn::Error::new(
            proc_macro2::Span::call_site(),
            "Scenario Outline missing Examples table",
        );
        return Err(err.to_compile_error().into());
    };

    let Some(table) = &first.table else {
        let err = syn::Error::new(
            proc_macro2::Span::call_site(),
            "Examples table missing rows",
        );
        return Err(err.to_compile_error().into());
    };

    let mut rows = table.rows.clone();
    if rows.is_empty() {
        let err = syn::Error::new(
            proc_macro2::Span::call_site(),
            "Examples table must have at least one row",
        );
        return Err(err.to_compile_error().into());
    }

    let headers = rows.remove(0);
    Ok(Some(ExampleTable { headers, rows }))
}

fn extract_scenario_steps(
    feature: &Feature,
    index: Option<usize>,
) -> Result<ScenarioData, TokenStream> {
    let Some(scenario) = feature.scenarios.get(index.unwrap_or(0)) else {
        let err = syn::Error::new(
            proc_macro2::Span::call_site(),
            "scenario index out of range",
        );
        return Err(err.to_compile_error().into());
    };

    let scenario_name = scenario.name.clone();
    let steps = scenario
        .steps
        .iter()
        .map(|s| {
            let keyword = match s.ty {
                StepType::Given => rstest_bdd::StepKeyword::Given,
                StepType::When => rstest_bdd::StepKeyword::When,
                StepType::Then => rstest_bdd::StepKeyword::Then,
            };
            (keyword, s.value.clone())
        })
        .collect();

    let examples = extract_examples(scenario)?;

    Ok(ScenarioData {
        name: scenario_name,
        steps,
        examples,
    })
}

fn extract_function_fixtures(
    sig: &Signature,
) -> (Vec<syn::Ident>, impl Iterator<Item = TokenStream2>) {
    let arg_idents: Vec<syn::Ident> = sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            FnArg::Typed(p) => match &*p.pat {
                Pat::Ident(id) => Some(id.ident.clone()),
                _ => None,
            },
            FnArg::Receiver(_) => None,
        })
        .collect();

    let inserts: Vec<_> = arg_idents
        .iter()
        .map(|id| quote! { ctx.insert(stringify!(#id), &#id); })
        .collect();

    (arg_idents, inserts.into_iter())
}

fn generate_case_attrs(examples: &ExampleTable) -> Vec<TokenStream2> {
    examples
        .rows
        .iter()
        .map(|row| {
            let cells = row.iter().map(|v| {
                let lit = syn::LitStr::new(v, proc_macro2::Span::call_site());
                quote! { #lit }
            });
            quote! { #[case( #(#cells),* )] }
        })
        .collect()
}

fn parameter_matches_header(arg: &FnArg, header: &str) -> bool {
    match arg {
        FnArg::Typed(p) => match &*p.pat {
            Pat::Ident(id) => id.ident == *header,
            _ => false,
        },
        FnArg::Receiver(_) => false,
    }
}

fn find_matching_parameter<'a>(
    sig: &'a mut Signature,
    header: &str,
) -> Result<&'a mut FnArg, TokenStream> {
    if let Some(pos) = sig
        .inputs
        .iter()
        .position(|arg| parameter_matches_header(arg, header))
    {
        sig.inputs
            .iter_mut()
            .nth(pos)
            .map_or_else(|| unreachable!("position from earlier search exists"), Ok)
    } else {
        Err(create_parameter_mismatch_error(sig, header))
    }
}

fn add_case_attribute_if_missing(arg: &mut FnArg) {
    if let FnArg::Typed(p) = arg {
        if !has_case_attribute(p) {
            p.attrs.push(syn::parse_quote!(#[case]));
        }
    }
}

fn has_case_attribute(p: &PatType) -> bool {
    p.attrs.iter().any(|attr| {
        let segs: Vec<_> = attr
            .path()
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect();
        segs == ["case"] || segs == ["rstest", "case"]
    })
}

fn create_parameter_mismatch_error(sig: &Signature, header: &str) -> TokenStream {
    let available_params: Vec<String> = sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            FnArg::Typed(p) => match &*p.pat {
                Pat::Ident(id) => Some(id.ident.to_string()),
                _ => None,
            },
            FnArg::Receiver(_) => None,
        })
        .collect();
    let msg = format!(
        "parameter `{header}` not found for scenario outline column. Available parameters: [{}]",
        available_params.join(", ")
    );
    syn::Error::new_spanned(sig, msg).to_compile_error().into()
}

fn process_scenario_outline_examples(
    sig: &mut Signature,
    examples: Option<&ExampleTable>,
) -> Result<(), TokenStream> {
    let Some(ex) = examples else {
        return Ok(());
    };
    for header in &ex.headers {
        let matching_param = find_matching_parameter(sig, header)?;
        add_case_attribute_if_missing(matching_param);
    }
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "signature defined by requirements",
)]
fn generate_scenario_code(
    attrs: &[Attribute],
    vis: &Visibility,
    sig: &Signature,
    block: &Block,
    feature_path_str: String,
    scenario_name: String,
    steps: Vec<(rstest_bdd::StepKeyword, String)>,
    examples: Option<ExampleTable>,
    ctx_inserts: impl Iterator<Item = TokenStream2>,
) -> TokenStream {
    let keywords: Vec<_> = steps
        .iter()
        .map(|(k, _)| match k {
            rstest_bdd::StepKeyword::Given => quote! { rstest_bdd::StepKeyword::Given },
            rstest_bdd::StepKeyword::When => quote! { rstest_bdd::StepKeyword::When },
            rstest_bdd::StepKeyword::Then => quote! { rstest_bdd::StepKeyword::Then },
            rstest_bdd::StepKeyword::And => quote! { rstest_bdd::StepKeyword::And },
            rstest_bdd::StepKeyword::But => quote! { rstest_bdd::StepKeyword::But },
        })
        .collect();
    let values = steps.iter().map(|(_, v)| v);

    let case_attrs = examples.map_or_else(Vec::new, |ex| generate_case_attrs(&ex));

    TokenStream::from(quote! {
        #[rstest::rstest]
        #(#case_attrs)*
        #(#attrs)*
        #vis #sig {
            let steps = [#((#keywords, #values)),*];
            let mut ctx = rstest_bdd::StepContext::default();
            #(#ctx_inserts)*
            for (index, (keyword, text)) in steps.iter().enumerate() {
                if let Some(f) = rstest_bdd::find_step(*keyword, (*text).into()) {
                    f(&ctx, text);
                } else {
                    panic!(
                        "Step not found at index {}: {} {} (feature: {}, scenario: {})",
                        index,
                        keyword.as_str(),
                        text,
                        #feature_path_str,
                        #scenario_name
                    );
                }
            }
            #block
        }
    })
}

pub(crate) fn scenario(attr: TokenStream, item: TokenStream) -> TokenStream {
    let ScenarioArgs { path, index } = parse_macro_input!(attr as ScenarioArgs);
    let path = PathBuf::from(path.value());

    let mut item_fn = parse_macro_input!(item as ItemFn);
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
