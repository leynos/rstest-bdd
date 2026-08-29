//! Return type classification for step wrappers.
//!
//! `rstest-bdd` step macros generate wrapper functions that normalize user step
//! return values into a common representation. The retained syntactic
//! classifier recognizes unit, never, and spelled `Result` paths during macro
//! expansion, but it cannot resolve local type aliases.
//!
//! Unhinted non-unit step returns therefore use runtime dispatch through
//! `rstest_bdd::step_return`. That bridge resolves the concrete return type:
//! an unhinted local alias of `Result<T, E>` dispatches as `Result`, while a
//! genuine value alias remains a value. Explicit `result` and `value` hints
//! retain their override roles, respectively forcing fallible normalization or
//! payload treatment.
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
//! These paths are a syntactic fast path only. Other unhinted non-unit return
//! types, including user-defined aliases, are classified by the runtime
//! dispatch bridge.

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

/// How a step wrapper normalizes a returned value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StepReturnStrategy {
    /// The step returns unit or has no explicit return type.
    Unit,
    /// The step diverges and needs no normalization.
    Never,
    /// The compiler selects either the `Result` or value normalization arm.
    Dispatch,
    /// The `value` hint forces payload treatment.
    ForcedValue,
    /// The `result` hint forces fallible treatment.
    ForcedResult,
}

/// Classify a step return type without guessing whether a named type is `Result`.
pub(crate) fn classify_step_return_type(
    output: &ReturnType,
    override_hint: Option<ReturnOverride>,
) -> syn::Result<StepReturnStrategy> {
    let ty = match output {
        ReturnType::Default => return Ok(StepReturnStrategy::Unit),
        ReturnType::Type(_, ty) => ty.as_ref(),
    };

    if is_unit_type(ty) {
        return Ok(StepReturnStrategy::Unit);
    }
    if matches!(ty, Type::Never(_)) {
        return Ok(StepReturnStrategy::Never);
    }

    match override_hint {
        Some(ReturnOverride::Value) => Ok(StepReturnStrategy::ForcedValue),
        Some(ReturnOverride::Result) => {
            if is_definitely_non_result_type(ty) {
                Err(syn::Error::new_spanned(
                    ty,
                    "return override `result` requires a return type shaped like `Result<T, E>` \
                     or `StepResult<T, E>`",
                ))
            } else {
                Ok(StepReturnStrategy::ForcedResult)
            }
        }
        None if is_nested_result_type(ty) => Err(syn::Error::new_spanned(
            ty,
            "unhinted step returns may not nest `Result`; use an explicit `value` or `result` hint",
        )),
        None if matches!(ty, Type::ImplTrait(_)) => Err(syn::Error::new_spanned(
            ty,
            "unhinted step returns may not use `impl Trait`; use an explicit `value` or `result` \
             hint",
        )),
        None => Ok(StepReturnStrategy::Dispatch),
    }
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
                        "return override `result` requires a return type shaped like `Result<T, \
                         E>` or `StepResult<T, E>`",
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

/// Classifies a syntactically recognized `Result`-like type.
fn classify_result_like(ty: &Type) -> Option<ReturnKind> {
    let path = match ty {
        Type::Path(type_path) => &type_path.path,
        _ => return None,
    };

    if is_result_like_path(path) {
        let ok_ty = first_type_argument(path)?;
        return Some(if is_unit_type(ok_ty) {
            ReturnKind::ResultUnit
        } else {
            ReturnKind::ResultValue
        });
    }

    None
}

/// Returns whether a syntactically known result has another result as its payload.
fn is_nested_result_type(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    is_result_like_path(&type_path.path)
        && first_type_argument(&type_path.path)
            .is_some_and(|inner| classify_result_like(inner).is_some())
}

/// Check if a type is the literal unit type `()`.
///
/// This only recognizes the syntactic `()` tuple; type aliases to unit
/// (e.g., `type UnitAlias = ()`) are *not* resolved at macro expansion time.
/// However, the runtime helper [`__rstest_bdd_payload_from_value`] identifies
/// unit aliases via `TypeId` comparison, so steps returning unit aliases will
/// still produce `None` payloads rather than boxed `()` values.
fn is_unit_type(ty: &Type) -> bool { matches!(ty, Type::Tuple(tuple) if tuple.elems.is_empty()) }

/// Returns whether a type can be ruled out as a `Result`-like type.
fn is_definitely_non_result_type(ty: &Type) -> bool {
    match ty {
        Type::Path(type_path) => {
            is_primitive_path(&type_path.path) || is_known_non_result_path(&type_path.path)
        }
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

/// Returns whether a path names a primitive type.
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

/// Returns whether a path names a known non-`Result` standard type.
fn is_known_non_result_path(path: &Path) -> bool {
    match_path_segments(path, |segments| {
        let segments: Vec<_> = segments.iter().map(String::as_str).collect();
        matches!(
            segments.as_slice(),
            ["String" | "Option" | "Vec"]
                | ["std" | "alloc", "string", "String"]
                | ["std" | "core", "option", "Option"]
                | ["std" | "alloc", "vec", "Vec"]
        )
    })
}

/// Returns whether an identifier names one of Rust's primitive types.
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

/// Returns `true` when `path` matches a recognized `Result` or `StepResult`
/// shape, combining both [`is_result_path`] and [`is_step_result_path`].
pub(crate) fn is_result_like_path(path: &Path) -> bool {
    is_result_path(path) || is_step_result_path(path)
}

/// Returns whether a path names a recognized standard `Result` type.
fn is_result_path(path: &Path) -> bool {
    matches_type_path(path, "Result", |segments| {
        let segments: Vec<_> = segments.iter().map(String::as_str).collect();
        matches!(segments.as_slice(), ["std" | "core", "result", "Result"])
    })
}

/// Returns whether a path names a recognized `StepResult` type.
fn is_step_result_path(path: &Path) -> bool {
    matches_type_path(path, "StepResult", |segments| {
        let segments: Vec<_> = segments.iter().map(String::as_str).collect();
        matches!(
            segments.as_slice(),
            ["rstest_bdd" | "crate" | "self" | "super", "StepResult"]
        )
    })
}

/// Extracts the first type argument from a generic path.
pub(crate) fn first_type_argument(path: &Path) -> Option<&Type> { nth_type_argument(path, 0) }

/// Extracts the error type `E` from `Result<T, E>` or `StepResult<T, E>`.
pub(crate) fn second_type_argument(path: &Path) -> Option<&Type> { nth_type_argument(path, 1) }

/// Extracts the type argument at position `n` from a generic path.
fn nth_type_argument(path: &Path, n: usize) -> Option<&Type> {
    let segment = path.segments.last()?;
    let args = match &segment.arguments {
        syn::PathArguments::AngleBracketed(args) => &args.args,
        _ => return None,
    };

    args.iter()
        .filter_map(|arg| match arg {
            syn::GenericArgument::Type(ty) => Some(ty),
            _ => None,
        })
        .nth(n)
}

#[cfg(test)]
mod tests;
