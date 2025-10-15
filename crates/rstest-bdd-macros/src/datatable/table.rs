//! Code generation for the `DataTable` derive macro.
//!
//! This module analyses tuple structs that wrap collections of rows and emits
//! conversions from the raw Gherkin data table into strongly typed wrappers.

mod attributes;

use attributes::{MapKind, TableConfig, parse_struct_attrs};

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    Data, DataStruct, DeriveInput, Field, Fields, GenericArgument, PathArguments, Type, TypePath,
    parse_macro_input, spanned::Spanned,
};

use crate::codegen::rstest_bdd_path;

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
            "#[derive(DataTable)] only supports tuple structs",
        ));
    };
    let field = extract_single_field(fields)?;
    let mut config = parse_struct_attrs(&input.attrs)?;
    let runtime = rstest_bdd_path();
    // NOTE: Row type inference only works for simple generic types. More complex
    // cases (for example nested generics or associated types) are not supported.
    // When inference fails callers must set `#[datatable(row = "Type")]`
    // explicitly.
    let (field_ty, row_ty_guess) = extract_inner_types(field);
    if config.row_ty.is_none() {
        config.row_ty = row_ty_guess;
    }
    let row_ty = config.row_ty.clone().ok_or_else(|| {
        syn::Error::new(
            field.span(),
            r#"unable to infer row type; specify #[datatable(row = "Type")]"#,
        )
    })?;
    let (builder, final_expr) = build_conversion(field, &field_ty, &config)?;
    let ident = &input.ident;
    let generics = input.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics ::core::convert::TryFrom<Vec<Vec<String>>> for #ident #ty_generics #where_clause {
            type Error = #runtime::datatable::DataTableError;

            fn try_from(table: Vec<Vec<String>>) -> Result<Self, Self::Error> {
                let rows = #runtime::datatable::Rows::<#row_ty>::try_from(table)?;
                #builder
                Ok(Self(#final_expr))
            }
        }
    })
}

fn extract_single_field(fields: &Fields) -> syn::Result<&Field> {
    if let Fields::Unnamed(unnamed) = fields {
        if unnamed.unnamed.len() == 1 {
            if let Some(field) = unnamed.unnamed.get(0) {
                return Ok(field);
            }
        }
    }
    Err(syn::Error::new(
        fields.span(),
        "#[derive(DataTable)] requires a tuple struct with a single field",
    ))
}

fn extract_inner_types(field: &Field) -> (Type, Option<Type>) {
    let Type::Path(TypePath { path, .. }) = &field.ty else {
        return (field.ty.clone(), None);
    };
    let Some(segment) = path.segments.last() else {
        return (field.ty.clone(), None);
    };
    if !is_supported_container(segment) {
        return (field.ty.clone(), None);
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return (field.ty.clone(), None);
    };
    let Some(GenericArgument::Type(inner)) = args.args.first() else {
        return (field.ty.clone(), None);
    };
    (field.ty.clone(), Some(inner.clone()))
}

fn is_supported_container(segment: &syn::PathSegment) -> bool {
    (segment.ident == "Rows" || segment.ident == "Vec")
        && matches!(segment.arguments, PathArguments::AngleBracketed(_))
}

fn build_conversion(
    field: &Field,
    field_ty: &Type,
    config: &TableConfig,
) -> syn::Result<(TokenStream2, TokenStream2)> {
    if let Some(map) = &config.map {
        let builder = match map {
            MapKind::Direct(func) => quote! { let value = #func(rows); },
            MapKind::Try(func) => quote! { let value = #func(rows)?; },
        };
        return Ok((builder, quote! { value }));
    }
    let Type::Path(TypePath { path, .. }) = field_ty else {
        return Err(syn::Error::new(
            field.span(),
            "#[derive(DataTable)] can only infer defaults for Rows<T> or Vec<T> fields",
        ));
    };
    let Some(segment) = path.segments.last() else {
        return Err(syn::Error::new(
            field.span(),
            "unsupported field type for #[derive(DataTable)]",
        ));
    };
    if segment.ident == "Rows" {
        Ok((quote! {}, quote! { rows }))
    } else if segment.ident == "Vec" {
        Ok((quote! { let value = rows.into_vec(); }, quote! { value }))
    } else {
        Err(syn::Error::new(
            field.span(),
            "unable to infer conversion; supply #[datatable(map = ..)]",
        ))
    }
}
