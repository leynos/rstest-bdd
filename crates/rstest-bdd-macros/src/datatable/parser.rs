use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::Type;

use crate::datatable::config::{Accessor, FieldConfig, FieldSpec};
use crate::datatable::validation::is_string_type;

pub(crate) fn accessor_expr(
    field: &FieldSpec,
    runtime: &TokenStream2,
    index: usize,
) -> TokenStream2 {
    let closure = parser_closure(&field.config, &field.inner_ty, runtime, index);
    match &field.config.accessor {
        Accessor::Column { name, .. } => {
            quote! { row.parse_column_with(#name, #closure) }
        }
        Accessor::Index { position, .. } => {
            let pos = syn::Index::from(*position);
            quote! { row.parse_with(#pos, #closure) }
        }
    }
}

pub(crate) fn parser_closure(
    config: &FieldConfig,
    target_ty: &Type,
    runtime: &TokenStream2,
    index: usize,
) -> TokenStream2 {
    let value_ident = format_ident!("cell_{index}");
    let mut statements = Vec::new();
    let mut current = quote! { #value_ident };
    if config.trim {
        let trimmed = format_ident!("trimmed_{index}");
        statements.push(quote! { let #trimmed = #current.trim(); });
        current = quote! { #trimmed };
    }
    let parse_expr = config.parse_with.as_ref().map_or_else(
        || {
            if config.truthy {
                quote! { #runtime::datatable::truthy_bool(#current) }
            } else if is_string_type(target_ty) {
                quote! { Ok::<#target_ty, ::core::convert::Infallible>(#current.to_owned()) }
            } else {
                quote! { #current.parse::<#target_ty>() }
            }
        },
        |parser| quote! { #parser(#current) },
    );
    quote! {
        |#value_ident| {
            #(#statements)*
            #parse_expr
        }
    }
}
