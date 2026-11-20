//! Tests for wrapper-local argument bindings.

use quote::format_ident;
use syn::parse_quote;

use super::{super::bindings as arg_bindings, *};

#[test]
fn collect_ordered_arguments_preserves_call_order() {
    let args = build_arguments();
    let names: Vec<String> = collect_ordered_arguments(&args)
        .into_iter()
        .map(|ident| ident.to_string())
        .collect();

    assert_eq!(
        names,
        [
            "rstest_bdd_arg_0",
            "rstest_bdd_arg_1",
            "rstest_bdd_arg_2",
            "rstest_bdd_arg_3"
        ]
    );
}

#[test]
fn wrapper_bindings_avoid_leading_underscores() {
    let fixture_name: syn::Ident = parse_quote!(state);
    let fixture_ty: syn::Type = parse_quote!(String);
    let binding = arg_bindings::wrapper_binding_ident(0);
    let ident: syn::Ident = parse_quote!(step_fn);
    let ctx_ident = format_ident!("__rstest_bdd_ctx");
    let tokens = gen_fixture_decls(
        &[BoundFixtureArg {
            arg: FixtureArg {
                name: &fixture_name,
                ty: &fixture_ty,
            },
            binding: &binding,
        }],
        &ident,
        &ctx_ident,
    );

    let declaration = tokens.first().expect("expected fixture declaration");
    let code = declaration.to_string();
    assert!(
        code.contains("rstest_bdd_arg_0"),
        "wrapper binding should avoid underscore prefix: {code}"
    );
    assert!(
        !code.contains("_state"),
        "fixture binding should not reuse the underscore identifier: {code}"
    );
}

#[test]
fn fixture_guard_idents_name_each_declared_guard_in_reverse_order() {
    let fixtures = [
        Arg::Fixture {
            pat: parse_quote!(first),
            name: parse_quote!(first),
            ty: parse_quote!(&u32),
        },
        Arg::Fixture {
            pat: parse_quote!(second),
            name: parse_quote!(second),
            ty: parse_quote!(&mut u32),
        },
    ];
    let fixture_refs: Vec<_> = fixtures.iter().collect();
    let bindings = build_bindings(fixture_refs.len());
    let bound = bind_fixture_args(&fixture_refs, &bindings);

    let guards: Vec<String> = fixture_guard_idents(&bound)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        guards,
        [
            "__rstest_bdd_guard_rstest_bdd_arg_1",
            "__rstest_bdd_guard_rstest_bdd_arg_0",
        ]
    );

    let ident: syn::Ident = parse_quote!(step_fn);
    let declarations = gen_fixture_decls(&bound, &ident, &format_ident!("__rstest_bdd_ctx"));
    for (guard, declaration) in guards.iter().rev().zip(&declarations) {
        let code = declaration.to_string();
        assert!(
            code.contains(&format!("{guard} =")),
            "{guard} not declared in {code}"
        );
    }
}
