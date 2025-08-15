//! Tests for argument extraction helpers.

use rstest::rstest;
use syn::parse_quote;

#[path = "../src/codegen/wrapper/args.rs"]
#[expect(dead_code, reason = "test reuses only selected helpers")]
// Procedural macro crates cannot expose non-macro items, so include directly.
mod args_impl;

use args_impl::{CallArg, extract_args};

#[rstest]
fn error_when_datatable_after_docstring() {
    let mut func: syn::ItemFn = parse_quote! {
        fn step(docstring: String, datatable: Vec<Vec<String>>) {}
    };
    let err = extract_args(&mut func)
        .err()
        .unwrap_or_else(|| panic!("expected error when datatable follows docstring"));
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
    let err = extract_args(&mut func)
        .err()
        .unwrap_or_else(|| panic!("expected error when datatable is declared twice"));
    assert!(
        err.to_string()
            .contains("only one datatable parameter is permitted"),
        "unexpected error message: {err}"
    );
}

#[rstest]
fn error_on_duplicate_docstring() {
    let mut func: syn::ItemFn = parse_quote! {
        fn step(docstring: String, docstring: String) {}
    };
    let err = extract_args(&mut func)
        .err()
        .unwrap_or_else(|| panic!("expected error when docstring is declared twice"));
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
    let err = extract_args(&mut func)
        .err()
        .unwrap_or_else(|| panic!("expected error when docstring has wrong type"));
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
    let args = extract_args(&mut func).unwrap_or_else(|e| panic!("failed to extract args: {e}"));
    assert_eq!(args.fixtures.len(), 1);
    let fixture = args
        .fixtures
        .first()
        .unwrap_or_else(|| panic!("missing fixture"));
    assert_eq!(fixture.name, "fixture");
}

#[rstest]
fn call_order_preserves_parameter_sequence() {
    use CallArg::{DataTable, DocString, Fixture, StepArg};
    let mut func: syn::ItemFn = parse_quote! {
        fn step(#[from] f: usize, a: i32, datatable: Vec<Vec<String>>, docstring: String, b: bool) {}
    };
    let args = extract_args(&mut func).unwrap_or_else(|e| panic!("failed to extract args: {e}"));
    assert!(matches!(
        &args.call_order[..],
        [Fixture(0), StepArg(0), DataTable, DocString, StepArg(1)]
    ));
}
