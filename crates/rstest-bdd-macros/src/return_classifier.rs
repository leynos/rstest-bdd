//! Return type classification for step wrappers.
//!
//! `rstest-bdd` step macros generate wrapper functions that normalise user step
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
//! are **not** resolved at macro expansion time; use the explicit `result` or
//! `value` hint on the step attribute when necessary.

use syn::{Path, ReturnType, Type};

/// How a step return value should be normalised by the generated wrapper.
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
        Some(ReturnOverride::Result) => classify_result_like(ty).ok_or_else(|| {
            syn::Error::new_spanned(
                ty,
                "return override `result` requires a return type shaped like `Result<T, E>` or `StepResult<T, E>`",
            )
        }),
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

fn is_result_path(path: &Path) -> bool {
    let segments: Vec<_> = path
        .segments
        .iter()
        .map(|seg| seg.ident.to_string())
        .collect();
    match segments.as_slice() {
        [single] => single == "Result",
        [root, module, leaf] => {
            (root == "std" || root == "core") && module == "result" && leaf == "Result"
        }
        _ => false,
    }
}

fn is_step_result_path(path: &Path) -> bool {
    let segments: Vec<_> = path
        .segments
        .iter()
        .map(|seg| seg.ident.to_string())
        .collect();
    match segments.as_slice() {
        [single] => single == "StepResult",
        [root, leaf] => {
            matches!(root.as_str(), "rstest_bdd" | "crate" | "self" | "super")
                && leaf == "StepResult"
        }
        _ => false,
    }
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

    #[test]
    fn classifies_unit_by_default() {
        let func: syn::ItemFn = syn::parse_quote!(
            fn step() {}
        );
        let kind = classify_return_type(&func.sig.output, None)
            .unwrap_or_else(|err| panic!("expected unit classification to succeed: {err}"));
        assert_eq!(kind, ReturnKind::Unit);
    }

    #[test]
    fn classifies_unit_tuple_return() {
        let func: syn::ItemFn = syn::parse_quote!(
            fn step() -> () {
                ()
            }
        );
        let kind = classify_return_type(&func.sig.output, None)
            .unwrap_or_else(|err| panic!("expected unit classification to succeed: {err}"));
        assert_eq!(kind, ReturnKind::Unit);
    }

    #[test]
    fn classifies_value_return() {
        let func: syn::ItemFn = syn::parse_quote!(
            fn step() -> u32 {
                1
            }
        );
        let kind = classify_return_type(&func.sig.output, None)
            .unwrap_or_else(|err| panic!("expected value classification to succeed: {err}"));
        assert_eq!(kind, ReturnKind::Value);
    }

    #[test]
    fn classifies_result_unit() {
        let func: syn::ItemFn = syn::parse_quote!(
            fn step() -> Result<(), &'static str> {
                Ok(())
            }
        );
        let kind = classify_return_type(&func.sig.output, None)
            .unwrap_or_else(|err| panic!("expected result-unit classification to succeed: {err}"));
        assert_eq!(kind, ReturnKind::ResultUnit);
    }

    #[test]
    fn classifies_result_value() {
        let func: syn::ItemFn = syn::parse_quote!(
            fn step() -> Result<u8, &'static str> {
                Ok(1)
            }
        );
        let kind = classify_return_type(&func.sig.output, None)
            .unwrap_or_else(|err| panic!("expected result-value classification to succeed: {err}"));
        assert_eq!(kind, ReturnKind::ResultValue);
    }

    #[test]
    fn recognises_std_result_path() {
        let func: syn::ItemFn = syn::parse_quote!(
            fn step() -> std::result::Result<u8, &'static str> {
                Ok(1)
            }
        );
        let kind = classify_return_type(&func.sig.output, None)
            .unwrap_or_else(|err| panic!("expected std::result::Result to classify: {err}"));
        assert_eq!(kind, ReturnKind::ResultValue);
    }

    #[test]
    fn recognises_core_result_path() {
        let func: syn::ItemFn = syn::parse_quote!(
            fn step() -> core::result::Result<(), &'static str> {
                Ok(())
            }
        );
        let kind = classify_return_type(&func.sig.output, None)
            .unwrap_or_else(|err| panic!("expected core::result::Result to classify: {err}"));
        assert_eq!(kind, ReturnKind::ResultUnit);
    }

    #[test]
    fn recognises_step_result() {
        let func: syn::ItemFn = syn::parse_quote!(
            fn step() -> StepResult<u8, &'static str> {
                Ok(1)
            }
        );
        let kind = classify_return_type(&func.sig.output, None)
            .unwrap_or_else(|err| panic!("expected StepResult to classify: {err}"));
        assert_eq!(kind, ReturnKind::ResultValue);
    }

    #[test]
    fn recognises_crate_step_result() {
        let func: syn::ItemFn = syn::parse_quote!(
            fn step() -> crate::StepResult<u8, &'static str> {
                Ok(1)
            }
        );
        let kind = classify_return_type(&func.sig.output, None)
            .unwrap_or_else(|err| panic!("expected crate::StepResult to classify: {err}"));
        assert_eq!(kind, ReturnKind::ResultValue);
    }

    #[test]
    fn recognises_super_step_result() {
        let func: syn::ItemFn = syn::parse_quote!(
            fn step() -> super::StepResult<u8, &'static str> {
                Ok(1)
            }
        );
        let kind = classify_return_type(&func.sig.output, None)
            .unwrap_or_else(|err| panic!("expected super::StepResult to classify: {err}"));
        assert_eq!(kind, ReturnKind::ResultValue);
    }

    #[test]
    fn override_value_forces_value() {
        let func: syn::ItemFn = syn::parse_quote!(
            fn step() -> Result<u8, &'static str> {
                Ok(1)
            }
        );
        let kind = classify_return_type(&func.sig.output, Some(ReturnOverride::Value))
            .unwrap_or_else(|err| panic!("expected override value to be accepted: {err}"));
        assert_eq!(kind, ReturnKind::Value);
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
