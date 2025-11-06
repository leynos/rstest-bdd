//! Expansion logic for `#[derive(StepArgs)]`.
//!
//! The derive macro targets structs with named fields and generates
//! implementations for [`rstest_bdd::step_args::StepArgs`] plus
//! [`TryFrom<Vec<String>>`]. Each field must implement [`FromStr`], enabling the
//! runtime wrapper to parse placeholder captures into the struct.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{spanned::Spanned, DeriveInput};

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

fn expand_named_struct(
    ident: &syn::Ident,
    mut generics: syn::Generics,
    fields: syn::FieldsNamed,
) -> syn::Result<TokenStream2> {
    let runtime = crate::codegen::rstest_bdd_path();

    let field_infos = named_struct_support::collect_field_infos(fields);

    if field_infos.is_empty() {
        return Err(syn::Error::new(
            ident.span(),
            "StepArgs structs must define at least one field",
        ));
    }

    named_struct_support::add_fromstr_bounds(&mut generics, &field_infos);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let field_count = field_infos.len();

    let parse_fields = named_struct_support::generate_field_parsing(&field_infos, &runtime);
    let field_idents: Vec<_> = field_infos.iter().map(|info| &info.ident).collect();
    let field_name_literals: Vec<_> = field_infos.iter().map(|info| info.name.clone()).collect();

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

mod named_struct_support {
    use proc_macro2::{Span, TokenStream as TokenStream2};
    use quote::quote;
    use syn::{parse_quote, FieldsNamed, Generics, Ident, LitStr, Type};

    pub(super) struct FieldInfo {
        pub ident: Ident,
        pub ty: Type,
        pub name: LitStr,
    }

    pub(super) fn collect_field_infos(fields: FieldsNamed) -> Vec<FieldInfo> {
        fields
            .named
            .into_iter()
            .filter_map(|field| field.ident.map(|ident| (ident, field.ty)))
            .map(|(ident, ty)| FieldInfo {
                name: LitStr::new(&ident.to_string(), Span::call_site()),
                ident,
                ty,
            })
            .collect()
    }

    pub(super) fn add_fromstr_bounds(generics: &mut Generics, infos: &[FieldInfo]) {
        let where_clause = generics.make_where_clause();
        for info in infos {
            let ty = &info.ty;
            where_clause
                .predicates
                .push(parse_quote!(#ty: ::core::str::FromStr));
        }
    }

    pub(super) fn generate_field_parsing(
        infos: &[FieldInfo],
        runtime: &TokenStream2,
    ) -> Vec<TokenStream2> {
        infos
            .iter()
            .map(|info| {
                let ident = &info.ident;
                let ty = &info.ty;
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
}

#[cfg(test)]
mod tests {
    use super::expand;
    use proc_macro2::TokenStream as TokenStream2;
    use quote::quote;
    use syn::DeriveInput;

    mod support_helpers {
        use super::super::named_struct_support;
        use quote::quote;
        use syn::{parse_quote, Generics};

        fn sample_fields() -> syn::FieldsNamed {
            parse_quote!({
                first: u32,
                second: String,
            })
        }

        #[test]
        fn collect_field_infos_returns_all_metadata() {
            let infos = named_struct_support::collect_field_infos(sample_fields());
            assert_eq!(infos.len(), 2);
            let Some(first) = infos.first() else {
                panic!("missing first field");
            };
            assert_eq!(first.ident.to_string(), "first");
            let Some(second) = infos.get(1) else {
                panic!("missing second field");
            };
            assert_eq!(second.name.value(), "second");
        }

        #[test]
        fn add_fromstr_bounds_adds_predicate_per_field() {
            let infos = named_struct_support::collect_field_infos(sample_fields());
            let mut generics = Generics::default();
            named_struct_support::add_fromstr_bounds(&mut generics, &infos);
            let predicate_len = generics
                .where_clause
                .as_ref()
                .map_or(0, |clause| clause.predicates.len());
            assert_eq!(predicate_len, infos.len());
        }

        #[test]
        fn generate_field_parsing_emits_code_for_each_field() {
            let infos = named_struct_support::collect_field_infos(sample_fields());
            let runtime = quote!(::rstest_bdd::runtime);
            let tokens = named_struct_support::generate_field_parsing(&infos, &runtime);
            assert_eq!(tokens.len(), infos.len());
            assert!(tokens
                .iter()
                .any(|ts| ts.to_string().contains("StepArgsError")));
        }
    }

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
