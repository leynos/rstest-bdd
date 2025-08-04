//! Step macro implementation and registration.

use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, LitStr, parse_macro_input};

use crate::args::extract_args;
use crate::codegen::{WrapperConfig, generate_wrapper_code};

pub(crate) fn step_attr(
    attr: TokenStream,
    item: TokenStream,
    keyword: rstest_bdd::StepKeyword,
) -> TokenStream {
    let pattern = parse_macro_input!(attr as LitStr);
    let mut func = parse_macro_input!(item as ItemFn);

    let (fixtures, step_args) = match extract_args(&mut func) {
        Ok(res) => res,
        Err(err) => return err.to_compile_error().into(),
    };

    {
        use std::collections::HashSet;
        let fixture_names: HashSet<_> = fixtures.iter().map(|f| &f.pat).collect();
        let step_arg_names: HashSet<_> = step_args.iter().map(|a| &a.pat).collect();
        let duplicates: Vec<_> = fixture_names.intersection(&step_arg_names).collect();
        if !duplicates.is_empty() {
            let dupes = duplicates
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            return syn::Error::new_spanned(
                &func.sig.ident,
                format!("Duplicate argument name(s) between fixtures and step arguments: {dupes}"),
            )
            .to_compile_error()
            .into();
        }
    }

    let ident = &func.sig.ident;

    let config = WrapperConfig {
        ident,
        fixtures: &fixtures,
        step_args: &step_args,
        pattern: &pattern,
        keyword,
    };
    let wrapper_code = generate_wrapper_code(&config);

    TokenStream::from(quote! {
        #func
        #wrapper_code
    })
}
