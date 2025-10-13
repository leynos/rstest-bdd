//! Code generation for the `DataTableRow` derive macro.
//!
//! The expander validates the annotated struct and emits a runtime
//! implementation capable of parsing rows into strongly typed values.

mod attributes;
mod bindings;

use attributes::{collect_fields, parse_struct_config};
use bindings::build_field_binding;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Data, DataStruct, DeriveInput, Generics, Type, parse_macro_input, spanned::Spanned};

use crate::codegen::rstest_bdd_path;
use crate::datatable::config::{Accessor, FieldConfig, FieldSpec};
use crate::datatable::validation::is_string_type;

pub(crate) fn expand(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_inner(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

fn expand_inner(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let Data::Struct(DataStruct { fields, .. }) = &input.data else {
        return Err(syn::Error::new(
            input.span(),
            "#[derive(DataTableRow)] only supports structs",
        ));
    };
    let struct_config = parse_struct_config(&input.attrs)?;
    let field_specs = collect_fields(fields, &struct_config)?;
    let runtime = rstest_bdd_path();
    let requires_header = field_specs
        .iter()
        .any(|field| matches!(field.config.accessor, Accessor::Column { .. }));
    let bindings: Vec<_> = field_specs
        .iter()
        .enumerate()
        .map(|(index, field)| build_field_binding(index, field, &runtime))
        .collect();
    let construct = build_constructor(&field_specs);
    let ident = &input.ident;
    let generics = augment_generics(&input.generics, &field_specs);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics #runtime::datatable::DataTableRow for #ident #ty_generics #where_clause {
            const REQUIRES_HEADER: bool = #requires_header;

            fn parse_row(row: #runtime::datatable::RowSpec<'_>) -> Result<Self, #runtime::datatable::DataTableError> {
                #(#bindings)*
                Ok(#construct)
            }
        }
    })
}

fn build_constructor(fields: &[FieldSpec]) -> TokenStream2 {
    fields
        .iter()
        .map(|field| field.ident.as_ref())
        .collect::<Option<Vec<_>>>()
        .map_or_else(
            || {
                let inits = fields.iter().enumerate().map(|(index, field)| {
                    let ident = field
                        .ident
                        .clone()
                        .unwrap_or_else(|| format_ident!("__field_{index}"));
                    quote! { #ident }
                });
                quote! { Self(#(#inits),*) }
            },
            |idents| {
                let inits = idents.iter().map(|ident| quote! { #ident });
                quote! { Self { #(#inits),* } }
            },
        )
}

fn augment_generics(generics: &Generics, fields: &[FieldSpec]) -> Generics {
    let mut generics = generics.clone();
    let where_clause = generics.make_where_clause();
    for field in fields {
        if needs_from_str_bound(&field.config, &field.inner_ty) {
            let ty = &field.inner_ty;
            where_clause.predicates.push(syn::parse_quote! {
                #ty: ::core::str::FromStr,
            });
            where_clause.predicates.push(syn::parse_quote! {
                <#ty as ::core::str::FromStr>::Err: ::std::error::Error + Send + Sync + 'static,
            });
        }
    }
    generics
}

fn needs_from_str_bound(config: &FieldConfig, inner_ty: &Type) -> bool {
    config.parse_with.is_none() && !config.truthy && !is_string_type(inner_ty)
}
