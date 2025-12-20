//! Attribute macro implementations.

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::parse_quote;

mod given;
mod scenario;
mod scenarios;
mod then;
mod when;

pub(crate) use given::given;
pub(crate) use scenario::scenario;
pub(crate) use scenarios::scenarios;
pub(crate) use then::then;
pub(crate) use when::when;

use crate::codegen::wrapper::args::ExtractedArgs;
use crate::codegen::wrapper::{WrapperConfig, extract_args, generate_wrapper_code};
use crate::return_classifier::{ReturnKind, ReturnOverride, classify_return_type};
use crate::utils::{
    errors::error_to_tokens,
    pattern::{infer_pattern, placeholder_names},
};

struct StepAttrArgs {
    pattern: Option<syn::LitStr>,
    return_override: Option<ReturnOverride>,
}

impl Parse for StepAttrArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(Self {
                pattern: None,
                return_override: None,
            });
        }

        if input.peek(syn::LitStr) {
            let pattern: syn::LitStr = input.parse()?;
            let return_override = if input.is_empty() {
                None
            } else {
                input.parse::<syn::Token![,]>()?;
                Some(parse_return_override(input)?)
            };
            if !input.is_empty() {
                return Err(input.error("unexpected tokens in step attribute"));
            }
            return Ok(Self {
                pattern: Some(pattern),
                return_override,
            });
        }

        let return_override = Some(parse_return_override(input)?);
        if !input.is_empty() {
            return Err(input.error("unexpected tokens in step attribute"));
        }
        Ok(Self {
            pattern: None,
            return_override,
        })
    }
}

fn parse_return_override(input: ParseStream<'_>) -> syn::Result<ReturnOverride> {
    let ident: syn::Ident = input.parse()?;
    match ident.to_string().as_str() {
        "result" => Ok(ReturnOverride::Result),
        "value" => Ok(ReturnOverride::Value),
        _ => Err(syn::Error::new_spanned(
            ident,
            "expected `result` or `value`",
        )),
    }
}

fn determine_step_pattern(pattern: Option<syn::LitStr>, ident: &syn::Ident) -> syn::LitStr {
    // TokenStream discards comments; a missing attribute or one containing only
    // whitespace infers the pattern from the function name. An explicit empty
    // string literal registers an empty pattern.
    pattern.map_or_else(
        || infer_pattern(ident),
        |lit| {
            let value = lit.value();
            if value.is_empty() {
                lit
            } else if value.trim().is_empty() {
                infer_pattern(ident)
            } else {
                lit
            }
        },
    )
}

fn extract_step_args_or_abort(
    func: &mut syn::ItemFn,
    unique_placeholders: &mut std::collections::HashSet<String>,
    keyword: crate::StepKeyword,
) -> ExtractedArgs {
    match extract_args(func, unique_placeholders) {
        Ok(args) => args,
        Err(err) => {
            let kw_name = match keyword {
                crate::StepKeyword::Given => "given",
                crate::StepKeyword::When => "when",
                crate::StepKeyword::Then => "then",
                crate::StepKeyword::And => "and",
                crate::StepKeyword::But => "but",
            };
            let help = format!(
                "Use `#[{kw_name}] fn name(...args...)` with supported step arguments/fixtures; remove self."
            );
            proc_macro_error::abort!(err.span(), "invalid step function signature: {}", err; help = help);
        }
    }
}

#[derive(Clone, Copy)]
struct WrapperInputs<'a> {
    func: &'a syn::ItemFn,
    pattern: &'a syn::LitStr,
    keyword: crate::StepKeyword,
    args: &'a ExtractedArgs,
    placeholder_names: &'a [syn::LitStr],
    return_kind: ReturnKind,
}

fn build_and_generate_wrapper(inputs: WrapperInputs<'_>) -> proc_macro2::TokenStream {
    let config = WrapperConfig {
        ident: &inputs.func.sig.ident,
        args: inputs.args,
        pattern: inputs.pattern,
        keyword: inputs.keyword,
        placeholder_names: inputs.placeholder_names,
        capture_count: inputs.placeholder_names.len(),
        return_kind: inputs.return_kind,
    };
    generate_wrapper_code(&config)
}

fn step_attr(attr: TokenStream, item: TokenStream, keyword: crate::StepKeyword) -> TokenStream {
    let mut func = syn::parse_macro_input!(item as syn::ItemFn);
    inject_skip_scope(&mut func);
    let attr_args = if attr.is_empty() {
        StepAttrArgs {
            pattern: None,
            return_override: None,
        }
    } else {
        syn::parse_macro_input!(attr as StepAttrArgs)
    };
    let pattern = determine_step_pattern(attr_args.pattern, &func.sig.ident);
    #[cfg(feature = "compile-time-validation")]
    #[cfg_attr(docsrs, doc(cfg(feature = "compile-time-validation")))]
    crate::validation::steps::register_step(keyword, &pattern);
    let mut placeholder_summary = match placeholder_names(&pattern.value()) {
        Ok(set) => set,
        Err(mut err) => {
            // Anchor diagnostics on the attribute literal for clarity.
            err.combine(syn::Error::new(pattern.span(), "in this step pattern"));
            return error_to_tokens(&err).into();
        }
    };

    let args = extract_step_args_or_abort(&mut func, &mut placeholder_summary.unique, keyword);

    let placeholder_literals: Vec<_> = placeholder_summary
        .ordered
        .iter()
        .map(|name| syn::LitStr::new(name, pattern.span()))
        .collect();
    let return_kind = classify_return_type(&func.sig.output, attr_args.return_override);

    let wrapper_code = build_and_generate_wrapper(WrapperInputs {
        func: &func,
        pattern: &pattern,
        keyword,
        args: &args,
        placeholder_names: &placeholder_literals,
        return_kind,
    });

    TokenStream::from(quote! {
        #func
        #wrapper_code
    })
}

/// Wraps a step function body with an RAII guard so the runtime can validate
/// every call to `skip!` against the current execution scope.
fn inject_skip_scope(func: &mut syn::ItemFn) {
    let path = crate::codegen::rstest_bdd_path();
    let ident = &func.sig.ident;
    let scope_init: syn::Stmt = parse_quote! {
        #[expect(unused_variables, reason = "RAII guard, only Drop matters")]
        let __rstest_bdd_step_scope_guard = #path::__rstest_bdd_enter_scope(
            #path::__rstest_bdd_scope_kind::Step,
            stringify!(#ident),
            file!(),
            line!(),
        );
    };
    let original_stmts = func.block.stmts.clone();
    *func.block = parse_quote!({
        #scope_init
        #(#original_stmts)*
    });
}
