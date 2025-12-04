//! Data table support helpers for wrapper argument generation.
//!
//! This module emits the per-wrapper snippets that pull the incoming
//! `Option<&[&[&str]]>` docstring/table arguments into concrete, typed values
//! expected by the step function. When a step declares a data table, we emit:
//! - Access to a wrapper-scoped table cache (see `emit::datatable_cache`) keyed
//!   by pointer identity plus an FNV-1a hash of contents.
//! - Conversion of the cached rows into the concrete type requested by the
//!   step signature.
//! - Diagnostics for cache misses to aid tests and telemetry.
//!   The `CacheIdents` passed in are the identifiers produced by the emitter
//!   for the key type and the cache `OnceLock<HashMap<...>>`, so we can
//!   reference them while generating argument initialisation code.

use super::super::args::{classify::is_cached_table, DataTableArg};
use super::{step_error_tokens, StepMeta};
use crate::codegen::wrapper::datatable_shared::{
    record_cache_miss_tokens, table_arc_tokens, table_content_match_tokens,
};
use proc_macro2::TokenStream as TokenStream2;
use quote::format_ident;

/// Identifiers for datatable caching infrastructure emitted alongside the wrapper.
pub(super) struct CacheIdents<'a> {
    /// Identifier of the wrapper-specific key struct used in the cache map.
    pub(super) key: &'a proc_macro2::Ident,
    /// Identifier of the `OnceLock<Mutex<HashMap<...>>>` storing cached tables.
    pub(super) cache: &'a proc_macro2::Ident,
}

/// Token fragments for cache entry operations.
struct CacheOperationTokens {
    content_match: TokenStream2,
    record_cache_miss: TokenStream2,
    arc_creation: TokenStream2,
}

fn datatable_error(
    pattern: &syn::LitStr,
    ident: &syn::Ident,
    message: &TokenStream2,
) -> TokenStream2 {
    step_error_tokens(&format_ident!("ExecutionError"), pattern, ident, message)
}

fn datatable_missing_error(pattern: &syn::LitStr, ident: &syn::Ident) -> TokenStream2 {
    datatable_error(
        pattern,
        ident,
        &quote::quote! { format!("Step '{}' requires a data table", #pattern) },
    )
}

fn datatable_convert_error(pattern: &syn::LitStr, ident: &syn::Ident) -> TokenStream2 {
    datatable_error(
        pattern,
        ident,
        &quote::quote! {
            format!(
                "failed to convert auxiliary argument for step '{}': {}",
                #pattern, e
            )
        },
    )
}

fn gen_cache_entry_match_tokens(
    cached_ident: &proc_macro2::Ident,
    table_ident: &proc_macro2::Ident,
    cache_ops: CacheOperationTokens,
) -> TokenStream2 {
    let CacheOperationTokens {
        content_match,
        record_cache_miss,
        arc_creation,
    } = cache_ops;
    quote::quote! {
        match guard.entry(key) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let #cached_ident = entry.get();
                let matches = if key.ptr == #table_ident.as_ptr() as usize {
                    true
                } else {
                    #content_match
                };

                if matches {
                    #cached_ident.clone()
                } else {
                    #record_cache_miss
                    let arc = #arc_creation;
                    entry.insert(arc.clone());
                    arc
                }
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                #record_cache_miss
                let arc = #arc_creation;
                entry.insert(arc.clone());
                arc
            }
        }
    }
}

fn gen_datatable_body(
    is_cached_table: bool,
    step_meta: StepMeta<'_>,
    cache_idents: &CacheIdents<'_>,
) -> TokenStream2 {
    let StepMeta { pattern, ident } = step_meta;
    let path = crate::codegen::rstest_bdd_path();
    let table_ident = format_ident!("table");
    let cached_ident = format_ident!("cached");
    let missing_err = datatable_missing_error(pattern, ident);
    let convert_err = datatable_convert_error(pattern, ident);
    let lock_err_message = quote::quote! {
        format!(
            "failed to access cached data table for step '{}': {}",
            #pattern,
            #path::datatable::DataTableError::CacheLockFailure
        )
    };
    let lock_err = datatable_error(pattern, ident, &lock_err_message);
    let key_ident = cache_idents.key;
    let cache_map_ident = cache_idents.cache;
    let conversion = if is_cached_table {
        quote::quote! { cached_table }
    } else {
        quote::quote! {
            {
                let owned: Vec<Vec<String>> = cached_table.into();
                owned
            }
            .try_into()
            .map_err(|e| #convert_err)?
        }
    };
    let arc_creation = table_arc_tokens(&table_ident);
    let content_match = table_content_match_tokens(&cached_ident, &table_ident);
    let record_cache_miss = record_cache_miss_tokens(&path);
    let cache_match = gen_cache_entry_match_tokens(
        &cached_ident,
        &table_ident,
        CacheOperationTokens {
            content_match,
            record_cache_miss,
            arc_creation,
        },
    );

    quote::quote! {
        let table = _table.ok_or_else(|| #missing_err)?;
        let key = #key_ident::new(table);
        let cache = #cache_map_ident.get_or_init(|| {
            std::sync::Mutex::new(std::collections::HashMap::new())
        });
        let arc_table = {
            let mut guard = cache.lock().map_err(|_| #lock_err)?;
            #cache_match
        };
        let cached_table = #path::datatable::CachedTable::from_arc(arc_table);
        #conversion
    }
}

/// Generate declaration for a data table argument.
///
/// Produces a `let` binding that extracts and caches the datatable from the
/// step's auxiliary arguments, performing content-based deduplication to reduce
/// allocations when the same table is reused across scenarios.
///
/// # Parameters
/// - `datatable`: The datatable argument metadata (pattern, type), if present.
/// - `step_meta`: Step metadata (pattern and function identifier) for error
///   reporting.
/// - `cache_idents`: Identifiers for the cache key type and cache storage
///   variable generated alongside the wrapper.
///
/// # Returns
/// `Some(TokenStream2)` containing the datatable binding, or `None` if no
/// datatable argument is present.
pub(super) fn gen_datatable_decl(
    datatable: Option<DataTableArg<'_>>,
    step_meta: StepMeta<'_>,
    cache_idents: &CacheIdents<'_>,
) -> Option<TokenStream2> {
    datatable.map(|arg| {
        let pat = arg.pat;
        let ty = arg.ty;
        let body = gen_datatable_body(is_cached_table(arg.ty), step_meta, cache_idents);
        quote::quote! {
            let #pat: #ty = {
                #body
            };
        }
    })
}
