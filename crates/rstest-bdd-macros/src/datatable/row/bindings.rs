use proc_macro2::{Ident, TokenStream as TokenStream2};
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
    if field.config.optional {
        build_optional_field_binding(binding_ident, accessor, runtime)
    } else if let Some(default) = &field.config.default {
        let default_expr = build_default_expr(default, &field.ty);
        build_field_binding_with_default(binding_ident, accessor, default_expr, runtime)
    } else {
        build_required_field_binding(binding_ident, accessor)
    }
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "helpers own token streams to simplify quoting logic"
)]
fn build_optional_field_binding(
    binding_ident: Ident,
    accessor: TokenStream2,
    runtime: &TokenStream2,
) -> TokenStream2 {
    build_binding_match(
        &binding_ident,
        accessor,
        runtime,
        quote! { Some(value) },
        quote! { None },
    )
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "helpers own token streams to simplify quoting logic"
)]
fn build_field_binding_with_default(
    binding_ident: Ident,
    accessor: TokenStream2,
    default_expr: TokenStream2,
    runtime: &TokenStream2,
) -> TokenStream2 {
    build_binding_match(
        &binding_ident,
        accessor,
        runtime,
        quote! { value },
        quote! { #default_expr },
    )
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "helpers own token streams to simplify quoting logic"
)]
fn build_required_field_binding(binding_ident: Ident, accessor: TokenStream2) -> TokenStream2 {
    quote! {
        let #binding_ident = #accessor?;
    }
}

fn build_default_expr(default: &DefaultValue, ty: &Type) -> TokenStream2 {
    match default {
        DefaultValue::Trait => quote! { <#ty as ::core::default::Default>::default() },
        DefaultValue::Function(path) => quote! { #path() },
    }
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "helpers own token streams to simplify quoting logic"
)]
fn build_binding_match(
    binding_ident: &Ident,
    accessor: TokenStream2,
    runtime: &TokenStream2,
    on_success: TokenStream2,
    on_missing: TokenStream2,
) -> TokenStream2 {
    let missing_pattern = missing_error_pattern(runtime);
    quote! {
        let #binding_ident = match #accessor {
            Ok(value) => #on_success,
            Err(err) => match err {
                #missing_pattern => #on_missing,
                _ => return Err(err),
            },
        };
    }
}

fn missing_error_pattern(runtime: &TokenStream2) -> TokenStream2 {
    quote! {
        #runtime::datatable::DataTableError::MissingColumn { .. }
        | #runtime::datatable::DataTableError::MissingCell { .. }
    }
}
