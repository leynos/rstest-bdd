//! Builds binding expressions for `#[derive(DataTableRow)]` fields.
//!
//! This module normalises how generated code fetches cell values from the
//! runtime `DataTable`, including graceful handling for optional and defaulted
//! fields. Optional members yield `None` when the source column is absent,
//! while defaults fall back to either `Default::default()` or a caller-supplied
//! function. All other fields bubble errors straight back to the caller so
//! derivations retain the existing failure semantics.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::Type;

use crate::datatable::config::{DefaultValue, FieldSpec};
use crate::datatable::parser::accessor_expr;

pub(crate) fn build_field_binding(
    index: usize,
    field: &FieldSpec,
    runtime: &TokenStream2,
) -> TokenStream2 {
    let binding_ident = field
        .ident
        .clone()
        .unwrap_or_else(|| format_ident!("__field_{index}"));
    let accessor = accessor_expr(field, runtime, index);
    let missing_pattern = missing_error_pattern(runtime);
    let on_missing = if field.config.optional {
        Some(quote! { None })
    } else if let Some(default) = &field.config.default {
        let expr = build_default_expr(default, &field.ty);
        Some(quote! { #expr })
    } else {
        None
    };

    on_missing.map_or_else(
        || {
            quote! {
                let #binding_ident = #accessor?;
            }
        },
        |on_missing| {
            let on_success = if field.config.optional {
                quote! { Some(value) }
            } else {
                quote! { value }
            };
            quote! {
                let #binding_ident = match #accessor {
                    Ok(value) => #on_success,
                    Err(err) => match err {
                        #missing_pattern => #on_missing,
                        _ => return Err(err),
                    },
                };
            }
        },
    )
}

fn build_default_expr(default: &DefaultValue, ty: &Type) -> TokenStream2 {
    match default {
        DefaultValue::Trait => quote! { <#ty as ::core::default::Default>::default() },
        DefaultValue::Function(path) => quote! { #path() },
    }
}

fn missing_error_pattern(runtime: &TokenStream2) -> TokenStream2 {
    quote! {
        #runtime::datatable::DataTableError::MissingColumn { .. }
        | #runtime::datatable::DataTableError::MissingCell { .. }
    }
}
