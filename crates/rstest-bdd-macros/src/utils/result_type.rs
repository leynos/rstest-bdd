//! Utilities for detecting `Result`-typed fixture parameters.
//!
//! Reuses the path-matching logic from [`crate::return_classifier`] to
//! recognize `Result<T, E>` and `StepResult<T, E>` shapes in fixture
//! parameter types. When a fixture returns a Result, the scenario prelude
//! can unwrap it with `?` and inject the inner `T` into the `StepContext`.

use syn::Type;

use crate::return_classifier::{first_type_argument, is_result_like_path, second_type_argument};

/// Recursively strips `Type::Paren` and `Type::Group` wrappers to reveal
/// the underlying type.
///
/// Parenthesized types like `(Result<T, E>)` and grouped types are
/// normalized to their inner form so Result detection works consistently.
pub(crate) fn ungroup_type(ty: &Type) -> &Type {
    match ty {
        Type::Paren(paren) => ungroup_type(&paren.elem),
        Type::Group(group) => ungroup_type(&group.elem),
        _ => ty,
    }
}

/// Returns `true` when the given type is a reference (`&` or `&mut`) whose
/// referent is a recognised `Result` or `StepResult` shape.
///
/// This detects `&Result<T, E>` and `&mut Result<T, E>` so that callers
/// can reject or special-case referenced Result fixtures rather than
/// silently treating them as plain references.
///
/// # Examples
///
/// ```rust,ignore
/// // &Result<MyWorld, String>      → true
/// // &mut Result<MyWorld, String>  → true
/// // &MyWorld                      → false
/// // Result<MyWorld, String>       → false
/// ```
pub(crate) fn is_referenced_result_type(ty: &Type) -> bool {
    let ty = ungroup_type(ty);
    let inner = match ty {
        Type::Reference(ref_ty) => ungroup_type(&ref_ty.elem),
        _ => return false,
    };
    let path = match inner {
        Type::Path(type_path) => &type_path.path,
        _ => return false,
    };
    is_result_like_path(path)
}

/// Internal helper: resolve the type argument selected by `getter` from a
/// recognised `Result` / `StepResult` type, or return `None` for any other
/// input.
fn extract_result_type_arg(ty: &Type, getter: fn(&syn::Path) -> Option<&Type>) -> Option<Type> {
    let ty = ungroup_type(ty);
    let path = match ty {
        Type::Path(type_path) => &type_path.path,
        _ => return None,
    };
    if !is_result_like_path(path) {
        return None;
    }
    getter(path).cloned()
}

/// Attempt to extract the inner `Ok` type from a `Result`-typed fixture
/// parameter.
///
/// Returns `Some(inner_type)` when the given type matches a recognised
/// `Result` or `StepResult` path, and `None` otherwise. The caller uses
/// the inner type to generate an unwrap statement and register the
/// fixture under the unwrapped type in `StepContext`.
///
/// # Examples
///
/// ```rust,ignore
/// // Result<MyWorld, String> → Some(MyWorld)
/// // MyWorld → None
/// // &mut MyWorld → None
/// ```
pub(crate) fn try_extract_result_inner_type(ty: &Type) -> Option<Type> {
    extract_result_type_arg(ty, first_type_argument)
}

/// Attempt to extract the error type `E` from a `Result<T, E>`-typed
/// fixture parameter.
///
/// Returns `Some(error_type)` when the given type matches a recognised
/// `Result` or `StepResult` path, and `None` otherwise. The caller uses
/// the error type to build a matching return type for the generated
/// scenario function (e.g. `-> Result<(), E>`).
///
/// # Examples
///
/// ```rust,ignore
/// // Result<MyWorld, String> → Some(String)
/// // Result<MyWorld, std::io::Error> → Some(std::io::Error)
/// // MyWorld → None
/// ```
pub(crate) fn try_extract_result_error_type(ty: &Type) -> Option<Type> {
    extract_result_type_arg(ty, second_type_argument)
}

#[cfg(test)]
#[expect(clippy::expect_used, reason = "test code uses infallible type parsing")]
mod tests {
    use super::*;
    use rstest::rstest;
    use syn::parse_quote;

    #[rstest]
    #[case("Result<MyWorld, String>", "MyWorld")]
    #[case("std::result::Result<Config, std::io::Error>", "Config")]
    #[case("StepResult<MyWorld, String>", "MyWorld")]
    fn extracts_inner_type_from_result_like(#[case] input: &str, #[case] expected: &str) {
        let ty = syn::parse_str::<Type>(input).expect("valid type");
        let inner = try_extract_result_inner_type(&ty);
        assert!(inner.is_some(), "should extract inner type from {input}");
        let inner_str = quote::quote! { #inner }.to_string();
        assert!(
            inner_str.contains(expected),
            "inner type should contain {expected}, got: {inner_str}"
        );
    }

    #[rstest]
    #[case("MyWorld")]
    #[case("&mut MyWorld")]
    #[case("Option<MyWorld>")]
    fn inner_type_returns_none_for_non_result(#[case] input: &str) {
        let ty = syn::parse_str::<Type>(input).expect("valid type");
        assert!(
            try_extract_result_inner_type(&ty).is_none(),
            "{input} should not be treated as Result"
        );
    }

    #[rstest]
    #[case("Result<MyWorld, String>", Some("String"))]
    #[case("std::result::Result<Config, std::io::Error>", Some("Error"))]
    #[case("StepResult<MyWorld, MyError>", Some("MyError"))]
    #[case("MyWorld", None)]
    fn extracts_error_type_from_result_like(#[case] input: &str, #[case] expected: Option<&str>) {
        let ty = syn::parse_str::<Type>(input).expect("valid type");
        let error = try_extract_result_error_type(&ty);
        match expected {
            Some(expected_str) => {
                assert!(error.is_some(), "should extract error type from {input}");
                let error_str = quote::quote! { #error }.to_string();
                assert!(
                    error_str.contains(expected_str),
                    "error type should contain {expected_str}, got: {error_str}"
                );
            }
            None => {
                assert!(error.is_none(), "{input} should not yield an error type");
            }
        }
    }

    // -- is_referenced_result_type tests ---

    #[test]
    fn detects_shared_ref_to_result() {
        let ty: Type = parse_quote! { &Result<MyWorld, String> };
        assert!(
            is_referenced_result_type(&ty),
            "&Result<T, E> should be detected as a referenced Result"
        );
    }

    #[test]
    fn detects_mut_ref_to_result() {
        let ty: Type = parse_quote! { &mut Result<MyWorld, String> };
        assert!(
            is_referenced_result_type(&ty),
            "&mut Result<T, E> should be detected as a referenced Result"
        );
    }

    #[test]
    fn detects_ref_to_std_result() {
        let ty: Type = parse_quote! { &std::result::Result<MyWorld, String> };
        assert!(
            is_referenced_result_type(&ty),
            "&std::result::Result should be detected as a referenced Result"
        );
    }

    #[test]
    fn detects_ref_to_step_result() {
        let ty: Type = parse_quote! { &StepResult<MyWorld, String> };
        assert!(
            is_referenced_result_type(&ty),
            "&StepResult should be detected as a referenced Result"
        );
    }

    #[test]
    fn ref_to_plain_type_is_not_referenced_result() {
        let ty: Type = parse_quote! { &MyWorld };
        assert!(
            !is_referenced_result_type(&ty),
            "&MyWorld should not be detected as a referenced Result"
        );
    }

    #[test]
    fn bare_result_is_not_referenced_result() {
        let ty: Type = parse_quote! { Result<MyWorld, String> };
        assert!(
            !is_referenced_result_type(&ty),
            "bare Result should not be detected as a referenced Result"
        );
    }

    #[test]
    fn plain_type_is_not_referenced_result() {
        let ty: Type = parse_quote! { MyWorld };
        assert!(
            !is_referenced_result_type(&ty),
            "plain type should not be detected as a referenced Result"
        );
    }

    // -- Parenthesized and grouped type tests ---

    #[test]
    fn parenthesized_result_extracts_inner_type() {
        let ty: Type = parse_quote! { (Result<MyWorld, String>) };
        let inner = try_extract_result_inner_type(&ty);
        assert!(
            inner.is_some(),
            "parenthesized Result should extract inner type"
        );
    }

    #[test]
    fn parenthesized_ref_result_is_detected() {
        let ty: Type = parse_quote! { &(Result<MyWorld, String>) };
        assert!(
            is_referenced_result_type(&ty),
            "parenthesized &Result should be detected as referenced Result"
        );
    }

    #[test]
    fn parenthesized_step_result_extracts_error() {
        let ty: Type = parse_quote! { (StepResult<MyWorld, MyError>) };
        let error = try_extract_result_error_type(&ty);
        assert!(
            error.is_some(),
            "parenthesized StepResult should extract error type"
        );
        let error_str = quote::quote! { #error }.to_string();
        assert!(
            error_str.contains("MyError"),
            "error type should be MyError, got: {error_str}"
        );
    }
}
