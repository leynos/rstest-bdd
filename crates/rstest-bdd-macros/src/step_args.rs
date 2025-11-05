//! Expansion logic for `#[derive(StepArgs)]`.
//!
//! The derive macro targets structs with named fields and generates
//! implementations for [`rstest_bdd::step_args::StepArgs`] plus
//! [`TryFrom<Vec<String>>`]. Each field must implement [`FromStr`], enabling the
//! runtime wrapper to parse placeholder captures into the struct.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{parse_quote, spanned::Spanned, DeriveInput};

pub(crate) fn derive(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    match expand(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

fn expand(input: DeriveInput) -> syn::Result<TokenStream2> {
    let DeriveInput {
        ident,
        generics,
        data,
        ..
    } = input;
    let syn::Data::Struct(struct_data) = data else {
        return Err(syn::Error::new(
            ident.span(),
            "StepArgs can only be derived for structs",
        ));
    };
    let syn::Fields::Named(fields) = struct_data.fields else {
        return Err(syn::Error::new(
            struct_data.struct_token.span(),
            "StepArgs requires named struct fields",
        ));
    };
    expand_named_struct(&ident, generics, fields)
}

fn collect_field_info(
    fields: syn::FieldsNamed,
) -> (Vec<syn::Ident>, Vec<syn::Type>, Vec<syn::LitStr>) {
    let mut field_idents = Vec::new();
    let mut field_types = Vec::new();
    let mut field_name_literals = Vec::new();

    for field in fields.named {
        let Some(field_ident) = field.ident else {
            continue;
        };
        field_name_literals.push(syn::LitStr::new(
            &field_ident.to_string(),
            Span::call_site(),
        ));
        field_types.push(field.ty);
        field_idents.push(field_ident);
    }

    (field_idents, field_types, field_name_literals)
}

fn add_fromstr_bounds(generics: &mut syn::Generics, field_types: &[syn::Type]) {
    let where_clause = generics.make_where_clause();
    for ty in field_types {
        where_clause
            .predicates
            .push(parse_quote!(#ty: ::core::str::FromStr));
    }
}

fn generate_field_parsing(
    field_idents: &[syn::Ident],
    field_types: &[syn::Type],
    runtime: &TokenStream2,
) -> Vec<TokenStream2> {
    field_idents
        .iter()
        .zip(field_types.iter())
        .map(|(ident, ty)| {
            quote! {
                let raw = values
                    .next()
                    .expect("value count verified before parsing");
                let #ident: #ty = match raw.parse::<#ty>() {
                    Ok(value) => value,
                    Err(_) => {
                        return Err(#runtime::step_args::StepArgsError::parse_failure(
                            stringify!(#ident),
                            &raw,
                        ));
                    }
                };
            }
        })
        .collect()
}

fn expand_named_struct(
    ident: &syn::Ident,
    mut generics: syn::Generics,
    fields: syn::FieldsNamed,
) -> syn::Result<TokenStream2> {
    let runtime = crate::codegen::rstest_bdd_path();

    let (field_idents, field_types, field_name_literals) = collect_field_info(fields);

    if field_idents.is_empty() {
        return Err(syn::Error::new(
            ident.span(),
            "StepArgs structs must define at least one field",
        ));
    }

    add_fromstr_bounds(&mut generics, &field_types);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let field_count = field_idents.len();

    let parse_fields = generate_field_parsing(&field_idents, &field_types, &runtime);

    let construct = quote! { Self { #(#field_idents),* } };

    Ok(quote! {
        impl #impl_generics #runtime::step_args::StepArgs for #ident #ty_generics #where_clause {
            const FIELD_COUNT: usize = #field_count;
            const FIELD_NAMES: &'static [&'static str] = &[#(#field_name_literals),*];

            fn from_captures(captures: Vec<String>) -> Result<Self, #runtime::step_args::StepArgsError> {
                if captures.len() != Self::FIELD_COUNT {
                    return Err(#runtime::step_args::StepArgsError::count_mismatch(
                        Self::FIELD_COUNT,
                        captures.len(),
                    ));
                }
                let mut values = captures.into_iter();
                #(#parse_fields)*
                Ok(#construct)
            }
        }

        impl #impl_generics ::std::convert::TryFrom<Vec<String>> for #ident #ty_generics #where_clause {
            type Error = #runtime::step_args::StepArgsError;

            fn try_from(value: Vec<String>) -> Result<Self, Self::Error> {
                <Self as #runtime::step_args::StepArgs>::from_captures(value)
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::expand;
    use proc_macro2::TokenStream as TokenStream2;
    use quote::quote;
    use syn::DeriveInput;

    fn expand_tokens(tokens: TokenStream2) -> syn::Result<TokenStream2> {
        let input = syn::parse2::<DeriveInput>(tokens)?;
        expand(input)
    }

    #[test]
    #[expect(clippy::expect_used, reason = "test asserts derive success path")]
    fn derives_step_args_for_named_struct() {
        let tokens = expand_tokens(quote! {
            struct AccountArgs {
                count: u32,
                label: String,
            }
        })
        .expect("derive should succeed");
        let rendered = tokens.to_string();
        assert!(
            rendered.contains("impl :: rstest_bdd :: step_args :: StepArgs for AccountArgs"),
            "StepArgs impl missing: {rendered}"
        );
        assert!(rendered.contains("const FIELD_COUNT : usize = 2"));
        assert!(rendered.contains("label"));
    }

    #[test]
    #[expect(clippy::expect_used, reason = "test asserts derive failure path")]
    fn rejects_tuple_structs() {
        let err = expand_tokens(quote! {
            struct TupleArgs(u32, String);
        })
        .expect_err("tuple structs should fail");
        assert!(err
            .to_string()
            .contains("StepArgs requires named struct fields"));
    }
}
