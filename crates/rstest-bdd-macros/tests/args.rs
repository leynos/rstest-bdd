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
fn error_when_datatable_after_docstring() {
    let mut func: syn::ItemFn = parse_quote! {
        fn step(docstring: String, datatable: Vec<Vec<String>>) {}
    };
    #[expect(clippy::expect_used, reason = "test asserts error message")]
    let err = extract_args(&mut func).expect_err("expected error when datatable follows docstring");
    let msg = err.to_string();
    assert!(
        msg.contains("datatable must be declared before docstring"),
        "unexpected error message: {msg}"
    );
}

#[rstest]
fn error_on_duplicate_datatable() {
    let mut func: syn::ItemFn = parse_quote! {
        fn step(datatable: Vec<Vec<String>>, datatable: Vec<Vec<String>>) {}
    };
    #[expect(clippy::expect_used, reason = "test asserts error message")]
    let err = extract_args(&mut func).expect_err("expected error when datatable is declared twice");
    assert!(
        err.to_string()
            .contains("only one datatable parameter is permitted"),
        "unexpected error message: {err}"
    );
}

#[rstest]
fn error_when_datatable_has_wrong_type() {
    let mut func: syn::ItemFn = parse_quote! {
        fn step(datatable: String) {}
    };
    #[expect(clippy::expect_used, reason = "test asserts error message")]
    let err = extract_args(&mut func).expect_err("expected error when datatable has wrong type");
    assert!(
        err.to_string().contains(
            "only one datatable parameter is permitted and it must have type `Vec<Vec<String>>`",
        ),
        "unexpected error message: {err}"
    );
}

#[rstest]
fn error_on_duplicate_docstring() {
    let mut func: syn::ItemFn = parse_quote! {
        fn step(docstring: String, docstring: String) {}
    };
    #[expect(clippy::expect_used, reason = "test asserts error message")]
    let err = extract_args(&mut func).expect_err("expected error when docstring is declared twice");
    assert!(
        err.to_string()
            .contains("only one docstring parameter is permitted"),
        "unexpected error message: {err}"
    );
}

#[rstest]
fn error_when_docstring_has_wrong_type() {
    let mut func: syn::ItemFn = parse_quote! {
        fn step(docstring: usize) {}
    };
    #[expect(clippy::expect_used, reason = "test asserts error message")]
    let err = extract_args(&mut func).expect_err("expected error when docstring has wrong type");
    assert!(
        err.to_string()
            .contains("only one docstring parameter is permitted"),
        "unexpected error message: {err}"
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
