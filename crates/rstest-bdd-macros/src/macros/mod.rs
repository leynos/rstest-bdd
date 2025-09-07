//! Attribute macro implementations.

use proc_macro::TokenStream;
use quote::quote;

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
use crate::utils::{
    errors::error_to_tokens,
    pattern::{infer_pattern, placeholder_names},
};

fn step_attr(attr: TokenStream, item: TokenStream, keyword: crate::StepKeyword) -> TokenStream {
    let mut func = syn::parse_macro_input!(item as syn::ItemFn);
    // TokenStream discards comments; a missing attribute or one containing only
    // whitespace infers the pattern from the function name. An explicit empty
    // string literal registers an empty pattern.
    let pattern = if attr.is_empty() {
        infer_pattern(&func.sig.ident)
    } else {
        let lit = syn::parse_macro_input!(attr as syn::LitStr);
        let value = lit.value();
        if value.is_empty() {
            lit
        } else if value.trim().is_empty() {
            infer_pattern(&func.sig.ident)
        } else {
            lit
        }
    };
    #[cfg(feature = "compile-time-validation")]
    #[cfg_attr(docsrs, doc(cfg(feature = "compile-time-validation")))]
    crate::validation::steps::register_step(keyword, &pattern);
    let mut placeholders = match placeholder_names(&pattern.value()) {
        Ok(set) => set,
        Err(mut err) => {
            // Anchor diagnostics on the attribute literal for clarity.
            err.combine(syn::Error::new(pattern.span(), "in this step pattern"));
            return error_to_tokens(&err).into();
        }
    };

    let args = match extract_args(&mut func, &mut placeholders) {
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

    let config = WrapperConfig {
        ident,
        fixtures: &args.fixtures,
        step_args: &args.step_args,
        datatable: args.datatable.as_ref(),
        docstring: args.docstring.as_ref(),
        pattern: &pattern,
        keyword,
        call_order: &args.call_order,
    };
    let wrapper_code = generate_wrapper_code(&config);

    TokenStream::from(quote! {
        #func
        #wrapper_code
    })
}
