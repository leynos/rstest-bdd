//! Utilities for handling fixtures in generated tests.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

/// Extract function argument identifiers and create insert statements.
pub(crate) fn extract_function_fixtures(
    sig: &syn::Signature,
) -> (Vec<syn::Ident>, impl Iterator<Item = TokenStream2>) {
    let arg_idents: Vec<syn::Ident> = sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(p) => match &*p.pat {
                syn::Pat::Ident(id) => Some(id.ident.clone()),
                _ => None,
            },
            syn::FnArg::Receiver(_) => None,
        })
        .collect();

    let inserts: Vec<_> = arg_idents
        .iter()
        .map(|id| quote! { ctx.insert(stringify!(#id), &#id); })
        .collect();

    (arg_idents, inserts.into_iter())
}
