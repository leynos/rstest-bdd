//! Unit tests for classifying step return types.

use super::{ReturnKind, ReturnOverride, classify_return_type, second_type_argument};

/// Helper to assert that a given function signature classifies to the expected kind.
fn assert_classifies_to(
    func_tokens: proc_macro2::TokenStream,
    override_hint: Option<ReturnOverride>,
    expected: ReturnKind,
) {
    let func: syn::ItemFn = match syn::parse2(func_tokens) {
        Ok(func) => func,
        Err(err) => panic!("test input should be valid function syntax: {err}"),
    };
    let kind = match classify_return_type(&func.sig.output, override_hint) {
        Ok(kind) => kind,
        Err(err) => panic!("expected classification to succeed: {err}"),
    };
    assert_eq!(kind, expected);
}

#[test]
fn classifies_unit_by_default() {
    assert_classifies_to(quote::quote! { fn step() {} }, None, ReturnKind::Unit);
}

#[test]
fn classifies_unit_tuple_return() {
    assert_classifies_to(
        quote::quote! { fn step() -> () { () } },
        None,
        ReturnKind::Unit,
    );
}

#[test]
fn classifies_value_return() {
    assert_classifies_to(
        quote::quote! { fn step() -> u32 { 1 } },
        None,
        ReturnKind::Value,
    );
}

#[test]
fn classifies_result_unit() {
    assert_classifies_to(
        quote::quote! { fn step() -> Result<(), &'static str> { Ok(()) } },
        None,
        ReturnKind::ResultUnit,
    );
}

#[test]
fn classifies_result_value() {
    assert_classifies_to(
        quote::quote! { fn step() -> Result<u8, &'static str> { Ok(1) } },
        None,
        ReturnKind::ResultValue,
    );
}

#[test]
fn recognizes_std_result_path() {
    assert_classifies_to(
        quote::quote! { fn step() -> std::result::Result<u8, &'static str> { Ok(1) } },
        None,
        ReturnKind::ResultValue,
    );
}

#[test]
fn recognizes_core_result_path() {
    assert_classifies_to(
        quote::quote! { fn step() -> core::result::Result<(), &'static str> { Ok(()) } },
        None,
        ReturnKind::ResultUnit,
    );
}

#[test]
fn recognizes_step_result() {
    assert_classifies_to(
        quote::quote! { fn step() -> StepResult<u8, &'static str> { Ok(1) } },
        None,
        ReturnKind::ResultValue,
    );
}

#[test]
fn recognizes_crate_step_result() {
    assert_classifies_to(
        quote::quote! { fn step() -> crate::StepResult<u8, &'static str> { Ok(1) } },
        None,
        ReturnKind::ResultValue,
    );
}

#[test]
fn recognizes_self_step_result() {
    assert_classifies_to(
        quote::quote! { fn step() -> self::StepResult<u8, &'static str> { Ok(1) } },
        None,
        ReturnKind::ResultValue,
    );
}

#[test]
fn recognizes_super_step_result() {
    assert_classifies_to(
        quote::quote! { fn step() -> super::StepResult<u8, &'static str> { Ok(1) } },
        None,
        ReturnKind::ResultValue,
    );
}

#[test]
fn recognizes_rstest_bdd_step_result() {
    assert_classifies_to(
        quote::quote! { fn step() -> rstest_bdd::StepResult<u8, &'static str> { Ok(1) } },
        None,
        ReturnKind::ResultValue,
    );
}

#[test]
fn override_value_forces_value() {
    assert_classifies_to(
        quote::quote! { fn step() -> Result<u8, &'static str> { Ok(1) } },
        Some(ReturnOverride::Value),
        ReturnKind::Value,
    );
}

#[test]
fn second_type_argument_extracts_error_type() {
    let ty: syn::Type = syn::parse_quote! { Result<u8, String> };
    let syn::Type::Path(tp) = &ty else {
        panic!("expected Type::Path")
    };
    let Some(second) = second_type_argument(&tp.path) else {
        panic!("should extract second type argument");
    };
    let s = quote::quote!(#second).to_string();
    assert!(s.contains("String"), "expected String, got: {s}");
}

#[test]
fn second_type_argument_returns_none_for_single_generic() {
    let ty: syn::Type = syn::parse_quote! { Option<u8> };
    let syn::Type::Path(tp) = &ty else {
        panic!("expected Type::Path")
    };
    assert!(second_type_argument(&tp.path).is_none());
}

#[test]
fn override_result_requires_result_like_return_type() {
    let func: syn::ItemFn = syn::parse_quote!(
        fn step() -> u8 {
            1
        }
    );
    let err = match classify_return_type(&func.sig.output, Some(ReturnOverride::Result)) {
        Ok(kind) => panic!("expected result override to fail, got {kind:?}"),
        Err(err) => err,
    };
    assert!(
        err.to_string()
            .contains("return override `result` requires"),
        "unexpected error: {err}"
    );
}
