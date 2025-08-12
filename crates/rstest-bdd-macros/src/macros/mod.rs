//! Attribute macro implementations.

use proc_macro::TokenStream;

mod given;
mod scenario;
mod then;
mod when;

pub(crate) use given::given;
pub(crate) use scenario::scenario;
pub(crate) use then::then;
pub(crate) use when::when;

use crate::codegen::wrapper::{WrapperConfig, extract_args, generate_wrapper_code};
use crate::utils::errors::error_to_tokens;

fn step_attr(
    attr: TokenStream,
    item: TokenStream,
    keyword: rstest_bdd::StepKeyword,
) -> TokenStream {
    let pattern = syn::parse_macro_input!(attr as syn::LitStr);
    let mut func = syn::parse_macro_input!(item as syn::ItemFn);

    let args = match extract_args(&mut func) {
        Ok(args) => args,
        Err(err) => return error_to_tokens(&err),
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

    TokenStream::from(quote::quote! {
        #func
        #wrapper_code
    })
}
