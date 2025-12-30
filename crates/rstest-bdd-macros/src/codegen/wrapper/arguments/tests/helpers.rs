//! Test helper functions for argument preparation tests.

use super::super::bindings;
use super::super::*;
use crate::codegen::wrapper::args::Arg;
use quote::quote;
use syn::parse_quote;

/// Create a sample `StepMeta` for testing.
pub fn sample_meta<'a>(pattern: &'a syn::LitStr, ident: &'a syn::Ident) -> StepMeta<'a> {
    StepMeta { pattern, ident }
}

/// Generate step parse code for a single argument with the given type and optional hint.
///
/// This helper encapsulates the common setup for testing `gen_step_parses`:
/// pattern creation, meta creation, argument/capture construction, and
/// token extraction. Returns the generated code as a string for assertions.
pub fn generate_step_parse_for_single_arg(ty: syn::Type) -> String {
    generate_step_parse_with_hint(ty, None)
}

/// Generate step parse code for a single argument with the given type and hint.
pub fn generate_step_parse_with_hint(ty: syn::Type, hint: Option<String>) -> String {
    let pattern: syn::LitStr = parse_quote!("test {name}");
    let ident: syn::Ident = parse_quote!(test_step);
    let meta = sample_meta(&pattern, &ident);

    let arg = Arg::Step {
        pat: parse_quote!(name),
        ty,
    };
    let binding = bindings::wrapper_binding_ident(0);
    let args = vec![BoundArg {
        arg: &arg,
        binding: &binding,
    }];
    let captures = vec![quote! { captures.get(0).map(|m| m.as_str()) }];
    let hints = vec![hint];

    let tokens = gen_step_parses(&args, &captures, &hints, meta);

    #[expect(
        clippy::expect_used,
        reason = "test helper asserts single token output"
    )]
    let token = tokens.first().expect("expected single token stream");
    token.to_string()
}

/// Build a standard set of test arguments covering all argument types.
pub fn build_arguments() -> Vec<Arg> {
    vec![
        Arg::Fixture {
            pat: parse_quote!(db),
            name: parse_quote!(db),
            ty: parse_quote!(String),
        },
        Arg::Step {
            pat: parse_quote!(count),
            ty: parse_quote!(usize),
        },
        Arg::DataTable {
            pat: parse_quote!(table),
            ty: parse_quote!(Vec<Vec<String>>),
        },
        Arg::DocString {
            pat: parse_quote!(doc),
        },
    ]
}
