//! Tests for argument extraction helpers.
use quote::quote;
use rstest::rstest;
use std::collections::HashSet;
use syn::parse_quote;
#[path = "../src/codegen/wrapper/args/mod.rs"]
#[expect(dead_code, reason = "test reuses only selected helpers")]
// Proc-macro crates cannot expose non-macro items to downstream crates; include
// the internal module directly to exercise helper APIs.
mod args_impl;
use args_impl::{Arg, ExtractedArgs, extract_args};
mod support;
use support::{
    find_datatable, fixture_count, has_docstring, ordered_parameter_names, step_arg_count,
};
/// Helper for invoking `extract_args` with placeholder names.
/// Consolidates repeated placeholder setup across tests.
fn test_extract_args_scenario(
    func_def: syn::ItemFn,
    placeholders: Vec<&str>,
) -> syn::Result<ExtractedArgs> {
    let mut func = func_def;
    let mut placeholder_set: HashSet<String> = placeholders.into_iter().map(String::from).collect();
    extract_args(&mut func, &mut placeholder_set)
}
#[rstest]
#[case(
    parse_quote! { fn step(docstring: String, datatable: Vec<Vec<String>>) {} },
    "DataTable must be declared before DocString",
    "error when datatable follows docstring",
)]
#[case(
    parse_quote! { fn step(datatable: Vec<Vec<String>>, datatable: Vec<Vec<String>>) {} },
    "only one DataTable parameter is permitted",
    "error on duplicate datatable",
)]
#[case(
    parse_quote! { fn step(datatable: String) {} },
    concat!(
        "parameter named `datatable` must have type `Vec<Vec<String>>` or `CachedTable` ",
        "(or use `#[datatable]` with a type that implements `TryFrom<Vec<Vec<String>>>`)",
    ),
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
    let err = extract_args(&mut func, &mut HashSet::new()).expect_err(test_description);
    let msg = err.to_string();
    assert!(
        msg.contains(expected_error_fragment),
        "unexpected error message for {test_description}: {msg}"
    );
}
#[rstest]
#[case(
    parse_quote! { fn step(docstring: String, #[datatable] data: Vec<Vec<String>>) {} },
    "DataTable must be declared before DocString",
    "error when datatable attribute follows docstring",
)]
#[case(
    parse_quote! { fn step(#[datatable] a: Vec<Vec<String>>, #[datatable] b: Vec<Vec<String>>) {} },
    "only one DataTable parameter is permitted",
    "error on multiple datatable parameters",
)]
#[case(
    parse_quote! { fn step(#[datatable] #[datatable] data: Vec<Vec<String>>) {} },
    "duplicate `#[datatable]` attribute",
    "error on duplicate datatable attribute",
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
fn test_datatable_attribute_errors(
    #[case] mut func: syn::ItemFn,
    #[case] expected_error_fragment: &str,
    #[case] description: &str,
) {
    #[expect(clippy::expect_used, reason = "test asserts error message")]
    let err = extract_args(&mut func, &mut HashSet::new()).expect_err(description);
    let msg = err.to_string();
    assert!(
        msg.contains(expected_error_fragment),
        "unexpected error message for {description}: {msg}"
    );
}

#[rstest]
fn from_without_ident_defaults_to_param_name() {
    let mut func: syn::ItemFn = parse_quote! {
        fn step(#[from] fixture: usize) {}
    };
    #[expect(clippy::expect_used, reason = "test asserts valid extraction")]
    let args = extract_args(&mut func, &mut HashSet::new()).expect("failed to extract args");
    assert_eq!(fixture_count(&args), 1);
    let Some(fixture_name) = args.args.iter().find_map(|arg| match arg {
        Arg::Fixture { name, .. } => Some(name.to_string()),
        _ => None,
    }) else {
        panic!("missing fixture");
    };
    assert_eq!(fixture_name, "fixture");
}

#[rstest]
fn malformed_from_attribute_reports_error() {
    let mut func: syn::ItemFn = parse_quote! {
        fn step(#[from(123)] fixture: usize) {}
    };
    #[expect(clippy::expect_used, reason = "test specifically checks error path")]
    let err =
        extract_args(&mut func, &mut HashSet::new()).expect_err("malformed #[from] should fail");
    assert!(
        err.to_string().contains("expected identifier"),
        "unexpected error: {err}"
    );
}

#[rstest]
fn call_order_preserves_parameter_sequence() {
    let mut func: syn::ItemFn = parse_quote! {
        fn step(#[from] f: usize, a: i32, datatable: Vec<Vec<String>>, docstring: String, b: bool) {}
    };
    let mut placeholders: HashSet<String> = ["a".into(), "b".into()].into_iter().collect();
    #[expect(clippy::expect_used, reason = "test asserts valid extraction")]
    let args = extract_args(&mut func, &mut placeholders).expect("failed to extract args");
    let ordered = ordered_parameter_names(&args);
    assert_eq!(ordered, ["f", "a", "datatable", "docstring", "b"]);
}

#[rstest]
fn datatable_attribute_recognised_and_preserves_type() {
    let mut func: syn::ItemFn = parse_quote! {
        fn step(#[datatable] table: my_mod::MyTable) {}
    };
    #[expect(clippy::expect_used, reason = "test asserts valid extraction")]
    let args = extract_args(&mut func, &mut HashSet::new()).expect("failed to extract args");
    #[expect(clippy::expect_used, reason = "datatable presence required")]
    let dt = find_datatable(&args).expect("missing datatable");
    if let Arg::DataTable { pat, ty } = dt {
        assert_eq!(pat.to_string(), "table");
        if let syn::Type::Path(tp) = ty {
            #[expect(clippy::expect_used, reason = "path has at least one segment")]
            let seg = tp.path.segments.last().expect("missing segment");
            assert_eq!(seg.ident, "MyTable");
            let rendered = tp
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            assert_eq!(rendered, "my_mod::MyTable");
        } else {
            panic!("expected path type");
        }
    } else {
        panic!("expected datatable argument");
    }
}

#[rstest]
fn datatable_attribute_removed_from_signature() {
    let mut func: syn::ItemFn = parse_quote! {
        fn step(#[datatable] data: Vec<Vec<String>>) {}
    };
    #[expect(clippy::expect_used, reason = "test asserts valid extraction")]
    let args = extract_args(&mut func, &mut HashSet::new()).expect("failed to extract args");
    #[expect(clippy::expect_used, reason = "datatable presence required")]
    let dt = find_datatable(&args).expect("missing datatable after strip");
    match dt {
        Arg::DataTable { pat, .. } => assert_eq!(pat.to_string(), "data"),
        _ => panic!("expected datatable argument"),
    }
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

#[test]
fn step_with_cached_table_is_classified_as_datatable() {
    let mut func: syn::ItemFn = parse_quote! {
        fn step(datatable: rstest_bdd::datatable::CachedTable) {}
    };

    #[expect(clippy::expect_used, reason = "test asserts valid extraction")]
    let args = extract_args(&mut func, &mut HashSet::new()).expect("failed to extract args");

    #[expect(clippy::expect_used, reason = "test requires datatable")]
    let dt = find_datatable(&args).expect("cached table should be classified as datatable");

    if let Arg::DataTable { ty, .. } = dt {
        let ty_str = quote!(#ty).to_string().replace(' ', "");
        assert!(
            ty_str.contains("CachedTable"),
            "unexpected datatable type: {ty_str}"
        );
    } else {
        panic!("expected datatable argument");
    }
}

#[rstest]
fn implicit_fixture_injected_without_from() {
    let func = parse_quote! { fn step(fixture: usize, count: u32) {} };
    #[expect(clippy::expect_used, reason = "test asserts valid extraction")]
    let args = test_extract_args_scenario(func, vec!["count"]).expect("failed to extract args");
    assert_eq!(fixture_count(&args), 1);
    assert_eq!(step_arg_count(&args), 1);
    assert_eq!(ordered_parameter_names(&args), ["fixture", "count"]);
}

#[rstest]
fn error_when_placeholder_missing_parameter() {
    let mut func: syn::ItemFn = parse_quote! {
        fn step(fixture: usize) {}
    };
    let mut placeholders: HashSet<String> = ["count".into()].into_iter().collect();
    #[expect(clippy::expect_used, reason = "test asserts error message")]
    let err = extract_args(&mut func, &mut placeholders).expect_err("missing placeholder");
    let msg = err.to_string();
    assert!(msg.contains("count"), "unexpected error: {msg}");
}

#[rstest]
fn placeholders_named_like_reserved_args_are_step_args() {
    let mut func: syn::ItemFn = parse_quote! {
        fn step(datatable: Vec<Vec<String>>, docstring: String) {}
    };
    let mut placeholders: HashSet<String> = ["datatable".into(), "docstring".into()]
        .into_iter()
        .collect();
    #[expect(clippy::expect_used, reason = "test asserts classification")]
    let args = extract_args(&mut func, &mut placeholders).expect("failed to extract args");
    assert_eq!(step_arg_count(&args), 2);
    assert!(find_datatable(&args).is_none());
    assert!(!has_docstring(&args));
}

#[rstest]
fn from_attribute_targets_placeholder() {
    let func = parse_quote! { fn step(#[from(count)] renamed: u32) {} };
    #[expect(clippy::expect_used, reason = "test asserts classification")]
    let args = test_extract_args_scenario(func, vec!["count"]).expect("failed to extract args");
    assert_eq!(fixture_count(&args), 0);
    assert_eq!(step_arg_count(&args), 1);
    assert_eq!(ordered_parameter_names(&args), ["renamed"]);
}

#[rstest]
fn underscore_prefixed_param_matches_placeholder() {
    let func = parse_quote! { fn step(_count: u32) {} };
    #[expect(clippy::expect_used, reason = "test asserts classification")]
    let args = test_extract_args_scenario(func, vec!["count"]).expect("extraction");
    assert_eq!(step_arg_count(&args), 1);
    assert_eq!(fixture_count(&args), 0);
    assert_eq!(ordered_parameter_names(&args), ["_count"]);
}

#[rstest]
fn underscore_prefixed_param_preserves_original_name() {
    let func = parse_quote! { fn step(_value: String, other: i32) {} };
    #[expect(clippy::expect_used, reason = "test asserts classification")]
    let args = test_extract_args_scenario(func, vec!["value", "other"]).expect("extraction");
    assert_eq!(step_arg_count(&args), 2);
    assert_eq!(ordered_parameter_names(&args), ["_value", "other"]);
}

#[test]
fn step_struct_argument_is_classified() {
    let mut func: syn::ItemFn = parse_quote! {
        fn step(#[step_args] params: OrderArgs, account: usize) {}
    };
    let mut placeholders: HashSet<String> = ["count".into(), "name".into()].into_iter().collect();
    #[expect(clippy::expect_used, reason = "test asserts classification")]
    let args = extract_args(&mut func, &mut placeholders).expect("failed to extract args");
    assert!(args.step_struct().is_some());
    assert_eq!(step_arg_count(&args), 0);
    assert_eq!(ordered_parameter_names(&args), ["params", "account"]);
}

#[rstest]
#[case(
    parse_quote! { fn step(#[step_args] params: OrderArgs, quantity: u32) {} },
    vec!["quantity"],
    "#[step_args] cannot be combined with named step arguments",
    "error when step arguments appear after #[step_args]",
)]
#[case(
    parse_quote! { fn step(#[step_args] params: OrderArgs) {} },
    vec![],
    "#[step_args] requires at least one placeholder",
    "error when placeholders missing",
)]
#[case(
    parse_quote! { fn step(#[step_args] params: &OrderArgs) {} },
    vec!["value"],
    "#[step_args] parameters must own their struct type",
    "error when step struct is a reference",
)]
#[case(
    parse_quote! { fn step(#[step_args] #[from(item)] params: OrderArgs) {} },
    vec!["item"],
    "#[step_args] cannot be combined with #[from]",
    "error when combining #[step_args] with #[from]",
)]
#[case(
    parse_quote! { fn step(#[step_args] #[step_args] params: OrderArgs) {} },
    vec!["value"],
    "duplicate `#[step_args]` attribute",
    "error on duplicate #[step_args] attribute",
)]
fn test_step_struct_errors(
    #[case] mut func: syn::ItemFn,
    #[case] placeholders: Vec<&str>,
    #[case] expected_error_fragment: &str,
    #[case] description: &str,
) {
    let mut placeholder_set: HashSet<String> = placeholders.into_iter().map(String::from).collect();
    #[expect(clippy::expect_used, reason = "test asserts error path")]
    let err = extract_args(&mut func, &mut placeholder_set).expect_err(description);
    let msg = err.to_string();
    assert!(
        msg.contains(expected_error_fragment),
        "unexpected error message for {description}: {msg}"
    );
}
