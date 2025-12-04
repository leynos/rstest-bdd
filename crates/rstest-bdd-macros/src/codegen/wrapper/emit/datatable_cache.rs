//! Datatable caching helpers used by wrapper emitters.

use crate::codegen::wrapper::datatable_shared::{
    cache_key_impl_tokens, cache_key_struct_tokens, cache_static_tokens,
};
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

pub(super) enum DatatableCacheComponents {
    None,
    Some {
        tokens: TokenStream2,
        key_ident: proc_macro2::Ident,
        cache_ident: proc_macro2::Ident,
    },
}

pub(super) fn generate_datatable_cache_definitions(
    has_datatable: bool,
    wrapper_ident: &proc_macro2::Ident,
) -> DatatableCacheComponents {
    if !has_datatable {
        return DatatableCacheComponents::None;
    }

    let key_ident = format_ident!("__rstest_bdd_table_key_{}", wrapper_ident);
    let cache_ident = format_ident!(
        "__RSTEST_BDD_TABLE_CACHE_{}",
        wrapper_ident.to_string().to_ascii_uppercase()
    );

    let key_struct = cache_key_struct_tokens(&key_ident);
    let key_impl = cache_key_impl_tokens(&key_ident);
    let cache_static = cache_static_tokens(&cache_ident, &key_ident);

    let tokens = quote! { #key_struct #key_impl #cache_static };
    DatatableCacheComponents::Some {
        tokens,
        key_ident,
        cache_ident,
    }
}
