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

    let body = match input.data {
        syn::Data::Struct(data) => match data.fields {
            syn::Fields::Named(fields) => expand_named(&fields.named)?,
            syn::Fields::Unnamed(fields) => expand_unnamed(&fields.unnamed)?,
            syn::Fields::Unit => ExpandParts {
                default: quote! { Self },
                reset: quote! {},
            },
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

    let ExpandParts { default, reset } = body;

    Ok(quote! {
        impl #impl_generics ::core::default::Default for #ident #ty_generics #where_clause {
            fn default() -> Self {
                #default
            }
        }

        impl #impl_generics #runtime::ScenarioState for #ident #ty_generics #where_clause {
            fn reset(&self) {
                #reset
            }
        }
    })
}

struct ExpandParts {
    default: TokenStream2,
    reset: TokenStream2,
}

fn expand_named(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>,
) -> syn::Result<ExpandParts> {
    let mut default_fields = Vec::with_capacity(fields.len());
    let mut reset_body = Vec::with_capacity(fields.len());

    for field in fields {
        let Some(ident) = &field.ident else {
            continue;
        };
        ensure_slot_type(field)?;
        default_fields.push(quote! { #ident: ::core::default::Default::default() });
        reset_body.push(quote! { self.#ident.clear(); });
    }

    Ok(ExpandParts {
        default: quote! { Self { #(#default_fields),* } },
        reset: quote! { #(#reset_body)* },
    })
}

fn expand_unnamed(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>,
) -> syn::Result<ExpandParts> {
    let mut defaults = Vec::with_capacity(fields.len());
    let mut resets = Vec::with_capacity(fields.len());

    for (index, field) in fields.iter().enumerate() {
        ensure_slot_type(field)?;
        let idx = syn::Index::from(index);
        defaults.push(quote! { ::core::default::Default::default() });
        resets.push(quote! { self.#idx.clear(); });
    }

    Ok(ExpandParts {
        default: quote! { Self( #(#defaults),* ) },
        reset: quote! { #(#resets)* },
    })
}

fn ensure_slot_type(field: &syn::Field) -> syn::Result<()> {
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
                    "ScenarioState fields must use Slot<T>",
                ))
            }
        }
        other => Err(syn::Error::new_spanned(
            other,
            "ScenarioState fields must be declared as Slot<T>",
        )),
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
    fn generates_default_and_reset_for_named_fields() {
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
        assert!(output.contains("impl :: core :: default :: Default for Example"));
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
                .contains("ScenarioState fields must use Slot")
        );
    }
}
