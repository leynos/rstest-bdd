//! Tests for argument extraction helpers.

use rstest::rstest;
use syn::parse_quote;

#[path = "../src/codegen/wrapper/args.rs"]
#[expect(dead_code, reason = "test reuses only selected helpers")]
// Proc-macro crates cannot export non-macro items, so the module is included
// directly.
mod args_impl;

use args_impl::{CallArg, extract_args};

#[rstest]
#[case(
    parse_quote! { fn step(docstring: String, datatable: Vec<Vec<String>>) {} },
    "datatable must be declared before docstring",
    "error when datatable follows docstring",
)]
#[case(
    parse_quote! { fn step(datatable: Vec<Vec<String>>, datatable: Vec<Vec<String>>) {} },
    "only one datatable parameter is permitted",
    "error on duplicate datatable",
)]
#[case(
    parse_quote! { fn step(datatable: String) {} },
    "only one datatable parameter is permitted and it must have type `Vec<Vec<String>>`",
    "error when datatable has wrong type",
)]
#[case(
    parse_quote! { fn step(docstring: String, docstring: String) {} },
    "only one docstring parameter is permitted",
    "error on duplicate docstring",
)]
#[case(
    parse_quote! { fn step(docstring: usize) {} },
    "only one docstring parameter is permitted and it must have type `String`",
    "error when docstring has wrong type",
)]
fn test_extract_args_errors(
    #[case] mut func: syn::ItemFn,
    #[case] expected_error_fragment: &str,
    #[case] test_description: &str,
) {
    #[expect(clippy::expect_used, reason = "test asserts error message")]
    let err = extract_args(&mut func).expect_err(test_description);
    let msg = err.to_string();
    assert!(
        msg.contains(expected_error_fragment),
        "unexpected error message: {msg}"
    );
}

#[rstest]
fn from_without_ident_defaults_to_param_name() {
    let mut func: syn::ItemFn = parse_quote! {
        fn step(#[from] fixture: usize) {}
    };
    #[expect(clippy::expect_used, reason = "test asserts valid extraction")]
    let args = extract_args(&mut func).expect("failed to extract args");
    assert_eq!(args.fixtures.len(), 1);
    #[expect(clippy::expect_used, reason = "fixture presence required")]
    let fixture = args.fixtures.first().expect("missing fixture");
    assert_eq!(fixture.name, "fixture");
}

#[rstest]
fn call_order_preserves_parameter_sequence() {
    use CallArg::{DataTable, DocString, Fixture, StepArg};
    let mut func: syn::ItemFn = parse_quote! {
        fn step(#[from] f: usize, a: i32, datatable: Vec<Vec<String>>, docstring: String, b: bool) {}
    };
    #[expect(clippy::expect_used, reason = "test asserts valid extraction")]
    let args = extract_args(&mut func).expect("failed to extract args");
    assert!(matches!(
        &args.call_order[..],
        [Fixture(0), StepArg(0), DataTable, DocString, StepArg(1)]
    ));
}
