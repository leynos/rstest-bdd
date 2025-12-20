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

use crate::codegen::wrapper::{WrapperConfig, extract_args, generate_wrapper_code};
use crate::return_classifier::{ReturnOverride, classify_return_type};
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
    // TokenStream discards comments; a missing attribute or one containing only
    // whitespace infers the pattern from the function name. An explicit empty
    // string literal registers an empty pattern.
    let pattern = match attr_args.pattern {
        None => infer_pattern(&func.sig.ident),
        Some(lit) => {
            let value = lit.value();
            if value.is_empty() {
                lit
            } else if value.trim().is_empty() {
                infer_pattern(&func.sig.ident)
            } else {
                lit
            }
        }
    };
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

    let args = match extract_args(&mut func, &mut placeholder_summary.unique) {
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
    };

    let ident = &func.sig.ident;
    let placeholder_literals: Vec<_> = placeholder_summary
        .ordered
        .iter()
        .map(|name| syn::LitStr::new(name, pattern.span()))
        .collect();
    let capture_count = placeholder_literals.len();
    let return_kind = classify_return_type(&func.sig.output, attr_args.return_override);

    let config = WrapperConfig {
        ident,
        args: &args,
        pattern: &pattern,
        keyword,
        placeholder_names: &placeholder_literals,
        capture_count,
        return_kind,
    };
    let wrapper_code = generate_wrapper_code(&config);

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
    func.block = Box::new(parse_quote!({
        #scope_init
        #(#original_stmts)*
    }));
}
