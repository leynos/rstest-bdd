//! Data table support helpers for wrapper argument generation.

use super::super::args::{classify::is_cached_table, DataTableArg};
use super::{step_error_tokens, StepMeta};
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

fn gen_arc_from_table() -> TokenStream2 {
    quote::quote! {
        std::sync::Arc::new(
            table
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

fn gen_content_match() -> TokenStream2 {
    quote::quote! {
        cached.len() == table.len()
            && cached
                .iter()
                .zip(table.iter())
                .all(|(cached_row, incoming_row)| {
                    cached_row.len() == incoming_row.len()
                        && cached_row
                            .iter()
                            .zip(incoming_row.iter())
                            .all(|(cached_cell, incoming_cell)| cached_cell == incoming_cell)
                })
    }
}

fn gen_datatable_body(
    is_cached_table: bool,
    step_meta: StepMeta<'_>,
    cache_idents: &CacheIdents<'_>,
) -> TokenStream2 {
    let StepMeta { pattern, ident } = step_meta;
    let path = crate::codegen::rstest_bdd_path();
    let missing_err = datatable_missing_error(pattern, ident);
    let convert_err = datatable_convert_error(pattern, ident);
    let key_ident = cache_idents.key;
    let cache_ident = cache_idents.cache;
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
    let arc_creation = gen_arc_from_table();
    let content_match = gen_content_match();

    quote::quote! {
        let table = _table.ok_or_else(|| #missing_err)?;
        let key = #key_ident::new(table);
        let cache = #cache_ident.get_or_init(|| {
            std::sync::Mutex::new(std::collections::HashMap::new())
        });
        let arc_table = {
            let mut guard = cache.lock().map_err(|e| #convert_err)?;
            match guard.entry(key) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    let cached = entry.get();
                    let matches = if key.ptr == table.as_ptr() as usize {
                        true
                    } else {
                        #content_match
                    };

                    if matches {
                        cached.clone()
                    } else {
                        #path::datatable::record_cache_miss();
                        let arc = #arc_creation;
                        entry.insert(arc.clone());
                        arc
                    }
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    #path::datatable::record_cache_miss();
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
