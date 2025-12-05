//! Datatable caching helpers used by wrapper emitters.
//!
//! Each generated wrapper gets a small cache storing an `Arc<Vec<Vec<String>>>`
//! version of the incoming `&[&[&str]]` data table. The cache is scoped per
//! wrapper function and keyed by a struct containing:
//! - The table pointer (for fast reuse when the exact slice is re-sent).
//! - An FNV-1a hash of the table contents (to detect identical-but-copied
//!   tables).
//!   `DatatableCacheComponents` returns the token fragments for the key type and
//!   the `OnceLock<Mutex<HashMap<key, Arc<_>>>>` cache. These are stitched into
//!   the wrapper body by `arguments::datatable`, which uses the cache to avoid
//!   re-cloning table contents on every call.

use crate::codegen::wrapper::datatable_shared::{
    cache_key_impl_tokens, cache_key_struct_tokens, cache_static_tokens,
};
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

/// Components generated for datatable caching infrastructure.
///
/// When a wrapper requires datatable support, this enum carries the generated
/// token stream and identifiers needed to wire up the caching mechanism.
pub(super) enum DatatableCacheComponents {
    /// No datatable caching required for this wrapper.
    None,
    /// Datatable caching components for wrappers with table parameters.
    Some {
        /// Combined token stream containing key struct, impl, and static cache.
        tokens: TokenStream2,
        /// Identifier for the generated cache key type.
        key_ident: proc_macro2::Ident,
        /// Identifier for the static cache variable.
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
