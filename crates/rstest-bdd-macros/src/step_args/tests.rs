//! Unit tests for step argument parsing.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::DeriveInput;

use super::expand;

fn expand_tokens(tokens: TokenStream2) -> syn::Result<TokenStream2> {
    let input = syn::parse2::<DeriveInput>(tokens)?;
    expand(input)
}

#[test]
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
fn rejects_tuple_structs() {
    let err = expand_tokens(quote! {
        struct TupleArgs(u32, String);
    })
    .expect_err("tuple structs should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("StepArgs requires named struct fields"),
        "unexpected error: {msg}"
    );
}

#[test]
fn rejects_invalid_step_args_field_attributes() {
    for (attribute, diagnostic) in [
        (
            quote!(#[step_args(placeholder = "first", placeholder = "second")]),
            "duplicate placeholder attribute",
        ),
        (quote!(#[step_args(trim, trim)]), "duplicate trim attribute"),
        (
            quote!(#[step_args(parse_with = parse_one, parse_with = parse_two)]),
            "duplicate parse_with attribute",
        ),
        (
            quote!(#[step_args(unsupported)]),
            "unsupported step_args field attribute",
        ),
    ] {
        let err = expand_tokens(quote! {
            struct InvalidArgs {
                #attribute
                value: String,
            }
        })
        .expect_err("invalid field attributes should fail");
        assert!(
            err.to_string().contains(diagnostic),
            "expected '{diagnostic}' in: {err}"
        );
    }
}
