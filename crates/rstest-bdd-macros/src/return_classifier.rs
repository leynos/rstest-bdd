//! Return type classification for step wrappers.
//!
//! `rstest-bdd` step macros generate wrapper functions that normalize user step
//! return values into a common representation. On stable Rust, we cannot rely
//! on overlapping trait impls (nor negative impls) to differentiate between
//! `T`, `()`, and `Result<..>`, so we perform best-effort classification during
//! macro expansion instead.
//!
//! ## Recognized Result paths
//!
//! The classifier recognizes these `Result` shapes:
//!
//! - `Result<..>` (bare name)
//! - `std::result::Result<..>` / `core::result::Result<..>`
//! - `StepResult<..>` (bare name)
//! - `rstest_bdd::StepResult<..>`, `crate::StepResult<..>`, `self::StepResult<..>`,
//!   `super::StepResult<..>`
//!
//! User-defined type aliases (e.g., `type MyResult<T> = Result<T, MyError>`)
//! are **not** resolved at macro expansion time. The explicit `result` hint
//! opts into a wrapper shape that expects `Result<..>` semantics and allows
//! aliases to compile as long as the return type is ultimately `Result`-like.

use syn::{Path, ReturnType, Type};

/// How a step return value should be normalized by the generated wrapper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReturnKind {
    /// The step returns `()` or has no explicit return type.
    Unit,
    /// The step returns a value `T` (boxed as `dyn Any`).
    Value,
    /// The step returns `Result<(), E>` and should propagate errors.
    ResultUnit,
    /// The step returns `Result<T, E>` and should propagate errors + payload.
    ResultValue,
}

/// Explicit override for return kind inference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReturnOverride {
    /// Force treating the return type as a `Result<..>`.
    Result,
    /// Force treating the return type as a value payload.
    Value,
}

/// Classify a function return type into one of the supported wrapper shapes.
pub(crate) fn classify_return_type(
    output: &ReturnType,
    override_hint: Option<ReturnOverride>,
) -> syn::Result<ReturnKind> {
    let ty = match output {
        ReturnType::Default => return Ok(ReturnKind::Unit),
        ReturnType::Type(_, ty) => ty.as_ref(),
    };

    if is_unit_type(ty) {
        return Ok(ReturnKind::Unit);
    }

    match override_hint {
        Some(ReturnOverride::Value) => Ok(ReturnKind::Value),
        Some(ReturnOverride::Result) => classify_result_like(ty).map_or_else(
            || {
                if is_definitely_non_result_type(ty) {
                    Err(syn::Error::new_spanned(
                        ty,
                        "return override `result` requires a return type shaped like `Result<T, E>` or `StepResult<T, E>`",
                    ))
                } else {
                    // We cannot resolve type aliases during macro expansion.
                    // Assume the return type behaves like `Result<T, E>` and let
                    // the compiler validate that the invoked step is actually
                    // result-like.
                    Ok(ReturnKind::ResultValue)
                }
            },
            Ok,
        ),
        None => Ok(classify_result_like(ty).unwrap_or(ReturnKind::Value)),
    }
}

fn classify_result_like(ty: &Type) -> Option<ReturnKind> {
    let path = match ty {
        Type::Path(type_path) => &type_path.path,
        _ => return None,
    };

    if is_result_path(path) || is_step_result_path(path) {
        let ok_ty = first_type_argument(path)?;
        return Some(if is_unit_type(ok_ty) {
            ReturnKind::ResultUnit
        } else {
            ReturnKind::ResultValue
        });
    }

    None
}

/// Check if a type is the literal unit type `()`.
///
/// This only recognizes the syntactic `()` tuple; type aliases to unit
/// (e.g., `type UnitAlias = ()`) are *not* resolved at macro expansion time.
/// However, the runtime helper [`__rstest_bdd_payload_from_value`] identifies
/// unit aliases via `TypeId` comparison, so steps returning unit aliases will
/// still produce `None` payloads rather than boxed `()` values.
fn is_unit_type(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(tuple) if tuple.elems.is_empty())
}

fn is_definitely_non_result_type(ty: &Type) -> bool {
    match ty {
        Type::Path(type_path) => is_primitive_path(&type_path.path),
        _ => true,
    }
}

/// Helper to extract path segments and apply a matching function.
fn match_path_segments<F>(path: &Path, matcher: F) -> bool
where
    F: FnOnce(&[String]) -> bool,
{
    let segments: Vec<_> = path
        .segments
        .iter()
        .map(|seg| seg.ident.to_string())
        .collect();
    matcher(segments.as_slice())
}

fn is_primitive_path(path: &Path) -> bool {
    match_path_segments(path, |segments| match segments {
        [single] => is_primitive_ident(single.as_str()),
        [root, module, leaf] => {
            (root == "std" || root == "core")
                && module == "primitive"
                && is_primitive_ident(leaf.as_str())
        }
        _ => false,
    })
}

fn is_primitive_ident(ident: &str) -> bool {
    const PRIMITIVE_IDENTS: &[&str] = &[
        "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
        "f32", "f64", "bool", "char", "str",
    ];
    PRIMITIVE_IDENTS.contains(&ident)
}

/// Match a path to a type by bare name or qualified path validator.
fn matches_type_path<F>(path: &Path, bare_name: &str, validate_qualified: F) -> bool
where
    F: Fn(&[String]) -> bool,
{
    match_path_segments(path, |segments| match segments {
        [single] if single == bare_name => true,
        qualified => validate_qualified(qualified),
    })
}

fn is_result_path(path: &Path) -> bool {
    matches_type_path(path, "Result", |segments| {
        let segments: Vec<_> = segments.iter().map(String::as_str).collect();
        matches!(segments.as_slice(), ["std" | "core", "result", "Result"])
    })
}

fn is_step_result_path(path: &Path) -> bool {
    matches_type_path(path, "StepResult", |segments| {
        let segments: Vec<_> = segments.iter().map(String::as_str).collect();
        matches!(
            segments.as_slice(),
            ["rstest_bdd" | "crate" | "self" | "super", "StepResult"]
        )
    })
}

fn first_type_argument(path: &Path) -> Option<&Type> {
    let segment = path.segments.last()?;
    let args = match &segment.arguments {
        syn::PathArguments::AngleBracketed(args) => &args.args,
        _ => return None,
    };

    args.iter().find_map(|arg| match arg {
        syn::GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::{ReturnKind, ReturnOverride, classify_return_type};

    /// Helper to assert that a given function signature classifies to the expected kind.
    fn assert_classifies_to(
        func_tokens: proc_macro2::TokenStream,
        override_hint: Option<ReturnOverride>,
        expected: ReturnKind,
    ) {
        let func: syn::ItemFn = syn::parse2(func_tokens).unwrap_or_else(|err| {
            panic!("test input should be valid function syntax: {err}");
        });
        let kind = classify_return_type(&func.sig.output, override_hint)
            .unwrap_or_else(|err| panic!("expected classification to succeed: {err}"));
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
}
