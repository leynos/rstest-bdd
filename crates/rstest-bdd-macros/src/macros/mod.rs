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

fn step_attr(attr: TokenStream, item: TokenStream, keyword: crate::StepKeyword) -> TokenStream {
    let pattern = syn::parse_macro_input!(attr as syn::LitStr);
    #[cfg(feature = "compile-time-validation")]
    #[cfg_attr(docsrs, doc(cfg(feature = "compile-time-validation")))]
    crate::validation::steps::register_step(keyword, &pattern);
    let mut func = syn::parse_macro_input!(item as syn::ItemFn);

    let args = match extract_args(&mut func) {
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
                "Use `#[{kw_name}] fn name(ctx: &rstest_bdd::StepContext, ...)` and valid fixtures."
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
