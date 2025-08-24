//! Tests for argument extraction helpers.

use quote::quote;
use rstest::rstest;
use syn::parse_quote;

#[path = "../src/codegen/wrapper/args.rs"]
#[expect(dead_code, reason = "test reuses only selected helpers")]
// Proc-macro crates cannot expose non-macro items to downstream crates; include
// the internal module directly to exercise helper APIs.
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
#[case(
    parse_quote! { fn step(docstring: String, #[datatable] data: Vec<Vec<String>>) {} },
    "datatable must be declared before docstring",
    "error when datatable attribute follows docstring",
)]
#[case(
    parse_quote! { fn step(#[datatable] a: Vec<Vec<String>>, #[datatable] b: Vec<Vec<String>>) {} },
    "only one datatable parameter is permitted",
    "error on duplicate datatable attributes",
)]
#[case(
    parse_quote! { fn step(#[datatable] docstring: String) {} },
    "parameter `docstring` cannot be annotated with #[datatable]",
    "error when docstring parameter uses datatable attribute",
)]
#[case(
    parse_quote! { fn step(#[from] #[datatable] fix: Vec<Vec<String>>) {} },
    "#[datatable] cannot be combined with #[from]",
    "error when datatable attribute applied to fixture",
)]
#[case(
    parse_quote! { fn step(#[datatable(foo)] data: Vec<Vec<String>>) {} },
    "`#[datatable]` does not take arguments",
    "error when datatable attribute has tokens",
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
        "unexpected error message for {test_description}: {msg}"
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

#[rstest]
fn datatable_attribute_recognised_and_preserves_type() {
    let mut func: syn::ItemFn = parse_quote! {
        fn step(#[datatable] table: MyTable) {}
    };
    #[expect(clippy::expect_used, reason = "test asserts valid extraction")]
    let args = extract_args(&mut func).expect("failed to extract args");
    #[expect(clippy::expect_used, reason = "datatable presence required")]
    let dt = args.datatable.expect("missing datatable");
    assert_eq!(dt.pat, "table");
    if let syn::Type::Path(tp) = &dt.ty {
        #[expect(clippy::expect_used, reason = "path has at least one segment")]
        let seg = tp.path.segments.last().expect("missing segment");
        assert_eq!(seg.ident, "MyTable");
    } else {
        panic!("expected path type");
    }
}

#[rstest]
fn datatable_attribute_removed_from_signature() {
    let mut func: syn::ItemFn = parse_quote! {
        fn step(#[datatable] data: Vec<Vec<String>>) {}
    };
    #[expect(clippy::expect_used, reason = "test asserts valid extraction")]
    let _args = extract_args(&mut func).expect("failed to extract args");
    #[expect(clippy::expect_used, reason = "test inspects parameter attributes")]
    let syn::FnArg::Typed(arg) = func.sig.inputs.first().expect("missing arg") else {
        panic!("expected typed argument");
    };
    assert!(arg.attrs.is_empty(), "datatable attribute not stripped");
    if let syn::Pat::Ident(p) = &*arg.pat {
        assert_eq!(p.ident, "data");
    } else {
        panic!("expected ident pattern");
    }
    let ty = &*arg.ty;
    let ty_str = quote!(#ty).to_string();
    assert!(
        ty_str.replace(' ', "") == "Vec<Vec<String>>".replace(' ', ""),
        "unexpected type after attribute strip: {ty_str}"
    );
}
