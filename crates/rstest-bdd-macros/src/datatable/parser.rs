//! Parsers for mapping datatable cells into typed values for step arguments.
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::Type;

use crate::{
    datatable::config::{Accessor, FieldSpec},
    named_fields::{ScalarConversion, scalar_parser_expression},
};

/// Provides the internal `accessor_expr` operation.
pub(crate) fn accessor_expr(
    field: &FieldSpec,
    runtime: &TokenStream2,
    index: usize,
) -> TokenStream2 {
    let closure = parser_closure(&field.config.conversion, &field.inner_ty, runtime, index);
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

/// Provides the internal `parser_closure` operation.
pub(crate) fn parser_closure(
    conversion: &ScalarConversion,
    target_ty: &Type,
    runtime: &TokenStream2,
    index: usize,
) -> TokenStream2 {
    let value_ident = format_ident!("cell_{index}");
    let parse_expr =
        scalar_parser_expression(conversion, quote! { #value_ident }, target_ty, runtime);
    quote! {
        |#value_ident| {
            #parse_expr
        }
    }
}
