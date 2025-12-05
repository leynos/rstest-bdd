//! Shared helpers for generating datatable caching code fragments.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

/// Tokens that convert an incoming table reference to an owned `Arc<Vec<Vec<String>>>`.
pub(super) fn table_arc_tokens(table_ident: &proc_macro2::Ident) -> TokenStream2 {
    quote! {
        std::sync::Arc::new(
            #table_ident
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|cell| cell.to_string())
                        .collect::<Vec<String>>()
                })
                .collect::<Vec<Vec<String>>>(),
        )
    }
}

/// Tokens that check whether cached and incoming tables share identical shape and contents.
pub(super) fn table_content_match_tokens(
    cached_ident: &proc_macro2::Ident,
    table_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    quote! {
        #cached_ident.len() == #table_ident.len()
            && #cached_ident
                .iter()
                .zip(#table_ident.iter())
                .all(|(cached_row, incoming_row)| {
                    cached_row.len() == incoming_row.len()
                        && cached_row
                            .iter()
                            .zip(incoming_row.iter())
                            .all(|(cached_cell, incoming_cell)| cached_cell == incoming_cell)
                })
    }
}

/// Tokens that record a datatable cache miss using the crate-local telemetry hook.
pub(super) fn record_cache_miss_tokens(path: &TokenStream2) -> TokenStream2 {
    quote! { #path::datatable::record_cache_miss(); }
}

/// Tokens that define the cache key struct capturing table pointer and hash.
pub(super) fn cache_key_struct_tokens(key_ident: &proc_macro2::Ident) -> TokenStream2 {
    quote! {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        struct #key_ident {
            ptr: usize,
            hash: u64,
        }
    }
}

/// Tokens that implement the cache key constructor with FNV-1a content hashing.
pub(super) fn cache_key_impl_tokens(key_ident: &proc_macro2::Ident) -> TokenStream2 {
    quote! {
        impl #key_ident {
            /// Compute FNV-1a hash of the table contents.
            fn compute_table_hash(table: &[&[&str]]) -> u64 {
                const FNV_OFFSET: u64 = 0xcbf29ce484222325;
                const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;

                let mut hash = FNV_OFFSET;
                for row in table {
                    for cell in *row {
                        hash ^= 0xff;
                        hash = hash.wrapping_mul(FNV_PRIME);
                        for byte in cell.as_bytes() {
                            hash ^= u64::from(*byte);
                            hash = hash.wrapping_mul(FNV_PRIME);
                        }
                    }
                    hash ^= 0xfe;
                    hash = hash.wrapping_mul(FNV_PRIME);
                }
                hash
            }

            fn new(table: &[&[&str]]) -> Self {
                let ptr = table.as_ptr() as usize;
                let hash = Self::compute_table_hash(table);
                Self { ptr, hash }
            }
        }
    }
}

/// Tokens that declare the static `OnceLock`-backed cache for datatable deduplication.
pub(super) fn cache_static_tokens(
    cache_ident: &proc_macro2::Ident,
    key_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    quote! {
        // Cache stores the runtime-owned table shape used by CachedTable::from_arc.
        static #cache_ident: std::sync::OnceLock<
            std::sync::Mutex<
                std::collections::HashMap<#key_ident, std::sync::Arc<Vec<Vec<String>>>>,
            >,
        > = std::sync::OnceLock::new();
    }
}
