//! Datatable caching helpers used by wrapper emitters.

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

fn gen_cache_key_struct(key_ident: &proc_macro2::Ident) -> TokenStream2 {
    quote! {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        struct #key_ident {
            ptr: usize,
            hash: u64,
        }
    }
}

fn gen_cache_key_impl(
    key_ident: &proc_macro2::Ident,
    hash_cache_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    quote! {
        impl #key_ident {
            /// Try to retrieve a cached hash for the given pointer.
            fn try_get_cached_hash(
                ptr: usize,
                cache: &std::sync::OnceLock<
                    std::sync::Mutex<std::collections::HashMap<usize, u64>>,
                >,
            ) -> Option<u64> {
                cache
                    .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
                    .lock()
                    .ok()?
                    .get(&ptr)
                    .copied()
            }

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

            /// Try to insert a computed hash into the cache.
            fn try_insert_hash(
                ptr: usize,
                hash: u64,
                cache: &std::sync::OnceLock<
                    std::sync::Mutex<std::collections::HashMap<usize, u64>>,
                >,
            ) {
                cache
                    .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
                    .lock()
                    .map(|mut guard| {
                        guard.insert(ptr, hash);
                    })
                    .ok();
            }

            fn new(table: &[&[&str]]) -> Self {
                static #hash_cache_ident: std::sync::OnceLock<
                    std::sync::Mutex<std::collections::HashMap<usize, u64>>,
                > = std::sync::OnceLock::new();

                let ptr = table.as_ptr() as usize;

                // Try cache lookup first
                if let Some(hash) = Self::try_get_cached_hash(ptr, &#hash_cache_ident) {
                    return Self { ptr, hash };
                }

                // Compute hash if not cached
                let hash = Self::compute_table_hash(table);

                // Cache for future lookups
                Self::try_insert_hash(ptr, hash, &#hash_cache_ident);

                Self { ptr, hash }
            }
        }
    }
}

fn gen_cache_static(
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
    let hash_cache_ident = format_ident!(
        "__RSTEST_BDD_TABLE_HASH_CACHE_{}",
        wrapper_ident.to_string().to_ascii_uppercase()
    );

    let key_struct = gen_cache_key_struct(&key_ident);
    let key_impl = gen_cache_key_impl(&key_ident, &hash_cache_ident);
    let cache_static = gen_cache_static(&cache_ident, &key_ident);

    let tokens = quote! { #key_struct #key_impl #cache_static };
    DatatableCacheComponents::Some {
        tokens,
        key_ident,
        cache_ident,
    }
}
