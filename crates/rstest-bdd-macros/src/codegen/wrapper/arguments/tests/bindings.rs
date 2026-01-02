//! Tests for wrapper-local argument bindings.

use super::super::bindings as arg_bindings;
use super::*;
use quote::format_ident;
use syn::parse_quote;

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

#[expect(clippy::expect_used, reason = "test asserts fixture declaration")]
#[test]
fn wrapper_bindings_avoid_leading_underscores() {
    let fixture = Arg::Fixture {
        pat: parse_quote!(_state),
        name: parse_quote!(state),
        ty: parse_quote!(String),
    };
    let binding = arg_bindings::wrapper_binding_ident(0);
    let ident: syn::Ident = parse_quote!(step_fn);
    let ctx_ident = format_ident!("__rstest_bdd_ctx");
    let tokens = gen_fixture_decls(
        &[BoundArg {
            arg: &fixture,
            binding: &binding,
        }],
        &ident,
        &ctx_ident,
    );

    let code = tokens.first().expect("expected fixture declaration");
    let code = code.to_string();
    assert!(
        code.contains("rstest_bdd_arg_0"),
        "wrapper binding should avoid underscore prefix: {code}"
    );
    assert!(
        !code.contains("_state"),
        "fixture binding should not reuse the underscore identifier: {code}"
    );
}
