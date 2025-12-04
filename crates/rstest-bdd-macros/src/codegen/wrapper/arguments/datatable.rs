//! Data table support helpers for wrapper argument generation.

use super::super::args::{classify::is_cached_table, DataTableArg};
use super::{step_error_tokens, StepMeta};
use crate::codegen::wrapper::datatable_shared::{
    record_cache_miss_tokens, table_arc_tokens, table_content_match_tokens,
};
use proc_macro2::TokenStream as TokenStream2;
use quote::format_ident;

/// Identifiers for datatable caching infrastructure.
pub(super) struct CacheIdents<'a> {
    pub(super) key: &'a proc_macro2::Ident,
    pub(super) cache: &'a proc_macro2::Ident,
}

fn datatable_missing_error(pattern: &syn::LitStr, ident: &syn::Ident) -> TokenStream2 {
    step_error_tokens(
        &format_ident!("ExecutionError"),
        pattern,
        ident,
        &quote::quote! { format!("Step '{}' requires a data table", #pattern) },
    )
}

fn datatable_convert_error(pattern: &syn::LitStr, ident: &syn::Ident) -> TokenStream2 {
    step_error_tokens(
        &format_ident!("ExecutionError"),
        pattern,
        ident,
        &quote::quote! { format!("failed to convert auxiliary argument for step '{}': {}", #pattern, e) },
    )
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

    quote::quote! {
        let table = _table.ok_or_else(|| #missing_err)?;
        let key = #key_ident::new(table);
        let cache = #cache_map_ident.get_or_init(|| {
            std::sync::Mutex::new(std::collections::HashMap::new())
        });
        let arc_table = {
            let mut guard = cache.lock().map_err(|e| #convert_err)?;
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
        };
        let cached_table = #path::datatable::CachedTable::from_arc(arc_table);
        #conversion
    }
}

/// Generate declaration for a data table argument.
pub(super) fn gen_datatable_decl(
    datatable: Option<DataTableArg<'_>>,
    step_meta: StepMeta<'_>,
    cache_idents: &CacheIdents<'_>,
) -> Option<TokenStream2> {
    datatable.map(|arg| {
        let pat = arg.pat.clone();
        let ty = arg.ty.clone();
        let body = gen_datatable_body(is_cached_table(arg.ty), step_meta, cache_idents);
        quote::quote! {
            let #pat: #ty = {
                #body
            };
        }
    })
}
