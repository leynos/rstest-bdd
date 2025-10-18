//! Expansion logic for the `#[derive(ScenarioState)]` macro.
//!
//! The macro accepts structs composed exclusively of [`Slot<T>`] fields and
//! implements [`ScenarioState`]. Named and tuple structs are both supported.
//! Unit structs implement `reset` trivially.
//!
//! Validation rejects enums, unions, and any field that is not a `Slot<T>`.
//! Error diagnostics include the offending field label to simplify fixes.
//!
//! [`Slot<T>`]: ::rstest_bdd::Slot
//! [`ScenarioState`]: ::rstest_bdd::ScenarioState

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{DeriveInput, spanned::Spanned};

pub(crate) fn derive(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    match expand(input) {
        Ok(stream) => stream.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

fn expand(input: DeriveInput) -> syn::Result<TokenStream2> {
    let ident = input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let runtime = crate::codegen::rstest_bdd_path();

    let reset = match input.data {
        syn::Data::Struct(data) => match data.fields {
            syn::Fields::Named(fields) => expand_named(&fields.named)?,
            syn::Fields::Unnamed(fields) => expand_unnamed(&fields.unnamed)?,
            syn::Fields::Unit => quote! {},
        },
        syn::Data::Enum(data) => {
            return Err(syn::Error::new(
                data.enum_token.span(),
                "ScenarioState can only be derived for structs",
            ));
        }
        syn::Data::Union(data) => {
            return Err(syn::Error::new(
                data.union_token.span(),
                "ScenarioState cannot be derived for unions",
            ));
        }
    };

    Ok(quote! {
        impl #impl_generics #runtime::ScenarioState for #ident #ty_generics #where_clause {
            fn reset(&self) {
                #reset
            }
        }
    })
}

fn expand_named(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>,
) -> syn::Result<TokenStream2> {
    let mut reset_body = Vec::with_capacity(fields.len());

    for field in fields {
        let Some(ident) = &field.ident else {
            continue;
        };
        ensure_slot_type(field, FieldLabel::Named(ident))?;
        reset_body.push(quote! { self.#ident.clear(); });
    }

    Ok(quote! { #(#reset_body)* })
}

fn expand_unnamed(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>,
) -> syn::Result<TokenStream2> {
    let mut resets = Vec::with_capacity(fields.len());

    for (index, field) in fields.iter().enumerate() {
        ensure_slot_type(field, FieldLabel::Unnamed(index))?;
        let idx = syn::Index::from(index);
        resets.push(quote! { self.#idx.clear(); });
    }

    Ok(quote! { #(#resets)* })
}

fn ensure_slot_type(field: &syn::Field, label: FieldLabel<'_>) -> syn::Result<()> {
    match &field.ty {
        syn::Type::Path(path) => {
            if path
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "Slot")
            {
                Ok(())
            } else {
                Err(syn::Error::new_spanned(
                    &field.ty,
                    format!(
                        "ScenarioState field '{}' must use Slot<T>",
                        label.describe()
                    ),
                ))
            }
        }
        other => Err(syn::Error::new_spanned(
            other,
            format!(
                "ScenarioState field '{}' must use Slot<T>",
                label.describe()
            ),
        )),
    }
}

#[derive(Copy, Clone)]
enum FieldLabel<'a> {
    Named(&'a syn::Ident),
    Unnamed(usize),
}

impl FieldLabel<'_> {
    fn describe(&self) -> String {
        match self {
            Self::Named(ident) => ident.to_string(),
            Self::Unnamed(index) => format!("#{index}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn expand_tokens(input: TokenStream2) -> syn::Result<TokenStream2> {
        let derive_input = syn::parse2::<DeriveInput>(input)?;
        expand(derive_input)
    }

    #[test]
    fn generates_reset_for_named_fields() {
        let tokens = match expand_tokens(quote! {
            struct Example {
                first: ::rstest_bdd::state::Slot<i32>,
                second: Slot<String>,
            }
        }) {
            Ok(tokens) => tokens,
            Err(err) => panic!("expected expansion: {err}"),
        };
        let output = tokens.to_string();
        assert!(output.contains("impl :: rstest_bdd :: ScenarioState for Example"));
        assert!(output.contains("self . first . clear ()"));
        assert!(output.contains("self . second . clear ()"));
    }

    #[test]
    fn rejects_non_slot_fields() {
        let err = match expand_tokens(quote! {
            struct Invalid {
                value: i32,
            }
        }) {
            Ok(tokens) => panic!("expected error, received tokens: {tokens}"),
            Err(err) => err,
        };
        assert!(
            err.to_string()
                .contains("ScenarioState field 'value' must use Slot<T>")
        );
    }
}
