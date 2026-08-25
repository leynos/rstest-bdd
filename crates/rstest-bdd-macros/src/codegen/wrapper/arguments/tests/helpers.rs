//! Test helper functions for argument preparation tests.

use quote::quote;
use syn::parse_quote;

use super::super::{bindings, *};
use crate::codegen::wrapper::args::Arg;

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
    let bindings = build_bindings(1);
    let args = bind_args(&[&arg], &bindings);
    let captures = vec![quote! { captures.get(0).map(|m| m.as_str()) }];
    let hints = vec![hint];

    let tokens = gen_step_parses(&args, &captures, &hints, meta);

    let [token] = tokens.as_slice() else {
        panic!("expected exactly one token stream, got {}", tokens.len());
    };
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

/// Generate wrapper-local binding identifiers for tests.
///
/// Bindings follow the `rstest_bdd_arg_N` format to mirror wrapper output.
pub fn build_bindings(count: usize) -> Vec<syn::Ident> {
    (0..count).map(bindings::wrapper_binding_ident).collect()
}

/// Pair each argument with its wrapper-local binding via `bind`.
///
/// The kind-specific wrappers below differ only in how they narrow an [`Arg`],
/// so the count check and the zip live here.
fn bind_arguments<'a, T>(
    args: &[&'a Arg],
    bindings: &'a [syn::Ident],
    bind: impl Fn(&'a Arg, &'a syn::Ident) -> T,
) -> Vec<T> {
    assert_binding_count(args.len(), bindings.len());
    args.iter()
        .zip(bindings.iter())
        .map(|(arg, binding)| bind(arg, binding))
        .collect()
}

/// Pair extracted fixture arguments with wrapper-local bindings for tests.
///
/// # Panics
/// Panics when the counts differ or an argument is not a fixture.
pub fn bind_fixture_args<'a>(
    args: &[&'a Arg],
    bindings: &'a [syn::Ident],
) -> Vec<BoundFixtureArg<'a>> {
    bind_arguments(args, bindings, |arg, binding| {
        let Some(fixture) = arg.as_fixture() else {
            panic!("bind_fixture_args expects fixture arguments, got {arg:?}");
        };
        BoundFixtureArg {
            arg: fixture,
            binding,
        }
    })
}

fn assert_binding_count(args: usize, bindings: usize) {
    assert!(
        args == bindings,
        "expected {args} bindings, got {bindings} bindings"
    );
}

/// Pair extracted step arguments with wrapper-local bindings for tests.
///
/// # Panics
/// Panics when the counts differ or an argument is not a step argument.
pub fn bind_args<'a>(args: &[&'a Arg], bindings: &'a [syn::Ident]) -> Vec<BoundStepArg<'a>> {
    bind_arguments(args, bindings, |arg, binding| {
        let Some(step) = arg.as_step() else {
            panic!("bind_args expects step arguments, got {arg:?}");
        };
        BoundStepArg { arg: step, binding }
    })
}
