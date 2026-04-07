//! Utilities for detecting `Result`-typed fixture parameters.
//!
//! Reuses the path-matching logic from [`crate::return_classifier`] to
//! recognize `Result<T, E>` and `StepResult<T, E>` shapes in fixture
//! parameter types. When a fixture returns a Result, the scenario prelude
//! can unwrap it with `?` and inject the inner `T` into the `StepContext`.

use syn::Type;

use crate::return_classifier::{first_type_argument, is_result_like_path};

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
    let path = match ty {
        Type::Path(type_path) => &type_path.path,
        _ => return None,
    };

    if !is_result_like_path(path) {
        return None;
    }

    first_type_argument(path).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn extracts_inner_type_from_bare_result() {
        let ty: Type = parse_quote! { Result<MyWorld, String> };
        let inner = try_extract_result_inner_type(&ty);
        assert!(inner.is_some(), "should extract inner type from Result");
        let inner_str = quote::quote! { #inner }.to_string();
        assert!(
            inner_str.contains("MyWorld"),
            "inner type should be MyWorld, got: {inner_str}"
        );
    }

    #[test]
    fn extracts_inner_type_from_std_result() {
        let ty: Type = parse_quote! { std::result::Result<Config, std::io::Error> };
        let inner = try_extract_result_inner_type(&ty);
        assert!(
            inner.is_some(),
            "should extract inner type from std::result::Result"
        );
    }

    #[test]
    fn extracts_inner_type_from_step_result() {
        let ty: Type = parse_quote! { StepResult<MyWorld, String> };
        let inner = try_extract_result_inner_type(&ty);
        assert!(inner.is_some(), "should extract inner type from StepResult");
    }

    #[test]
    fn returns_none_for_plain_type() {
        let ty: Type = parse_quote! { MyWorld };
        assert!(
            try_extract_result_inner_type(&ty).is_none(),
            "plain type should not be treated as Result"
        );
    }

    #[test]
    fn returns_none_for_reference_type() {
        let ty: Type = parse_quote! { &mut MyWorld };
        assert!(
            try_extract_result_inner_type(&ty).is_none(),
            "reference type should not be treated as Result"
        );
    }

    #[test]
    fn returns_none_for_option_type() {
        let ty: Type = parse_quote! { Option<MyWorld> };
        assert!(
            try_extract_result_inner_type(&ty).is_none(),
            "Option should not be treated as Result"
        );
    }
}
