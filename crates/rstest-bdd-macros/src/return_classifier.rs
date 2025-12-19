//! Return type classification for step wrappers.
//!
//! `rstest-bdd` step macros generate wrapper functions that normalise user step
//! return values into a common representation. On stable Rust, we cannot rely
//! on overlapping trait impls (nor negative impls) to differentiate between
//! `T`, `()`, and `Result<..>`, so we perform best-effort classification during
//! macro expansion instead.

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
                "return kind override `result` requires the return type to be `Result<..>` or `StepResult<..>`",
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

fn is_unit_type(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(tuple) if tuple.elems.is_empty())
}

fn is_result_path(path: &Path) -> bool {
    let segments: Vec<_> = path.segments.iter().map(|seg| seg.ident.to_string()).collect();
    matches!(
        segments.as_slice(),
        ["Result"] | ["std", "result", "Result"] | ["core", "result", "Result"]
    )
}

fn is_step_result_path(path: &Path) -> bool {
    let segments: Vec<_> = path.segments.iter().map(|seg| seg.ident.to_string()).collect();
    matches!(segments.as_slice(), ["StepResult"] | ["rstest_bdd", "StepResult"])
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

