//! Type- and name-shape predicates for the reserved step parameters.
//!
//! Recognizing `DataTable` and `DocString` parameters is purely a question of
//! the parameter's identifier and type, with no access to classification state.
//! Keeping these predicates separate from the classifiers that consult them
//! holds the parent module within the per-file line budget and gives the shape
//! rules one place to live.
//!
//! `is_cached_table` deliberately stays in the parent module: it is the one
//! predicate re-exported beyond this subtree, and some test harnesses compile
//! the parent without the table-binding code that consumes it, which would make
//! a re-export here read as unused.

use super::is_cached_table;

/// Test whether `ty` is a chain of single-generic types naming `seq` in order.
///
/// Walks the type's nested first generic argument once per entry in `seq`,
/// comparing each against the *final* path segment, so both `String` and
/// `std::string::String` match `["String"]`. The last entry is the innermost
/// type and is the only one permitted to have no generic arguments: `seq`
/// `["Vec", "Vec", "String"]` matches `Vec<Vec<String>>` but not `Vec<Vec<_>>`
/// with a different leaf, nor a longer chain that runs out of arguments early.
///
/// Returns `false` for any non-path type (references, tuples, slices) rather
/// than looking through it.
pub(super) fn is_type_seq(ty: &syn::Type, seq: &[&str]) -> bool {
    use syn::{GenericArgument, PathArguments, Type};

    let mut cur = ty;
    for (i, &name) in seq.iter().enumerate() {
        let Type::Path(tp) = cur else { return false };
        let Some(segment) = tp.path.segments.last() else {
            return false;
        };
        if segment.ident != name {
            return false;
        }
        match &segment.arguments {
            PathArguments::AngleBracketed(ab) if !ab.args.is_empty() => {
                if let Some(GenericArgument::Type(inner)) = ab.args.get(0) {
                    cur = inner;
                    continue;
                }
                return false;
            }
            _ => {
                if i + 1 != seq.len() {
                    return false;
                }
            }
        }
    }
    true
}

/// Test whether `ty` is `String`, the only type a `DocString` parameter accepts.
pub(super) fn is_string(ty: &syn::Type) -> bool {
    is_type_seq(ty, &["String"])
}

/// Test whether `ty` is the raw `Vec<Vec<String>>` table representation.
pub(super) fn is_datatable(ty: &syn::Type) -> bool {
    is_type_seq(ty, &["Vec", "Vec", "String"])
}

/// Test whether a parameter is the canonical `DataTable` shape.
///
/// True only when the identifier is literally `datatable` *and* the type is one
/// of the two supported table representations. A parameter named `datatable`
/// with any other type is deliberately not a match here, so
/// [`classify_datatable`] can reject it with a type-specific diagnostic rather
/// than passing it to the next classifier.
pub(super) fn should_classify_as_datatable(pat: &syn::Ident, ty: &syn::Type) -> bool {
    pat == "datatable" && (is_datatable(ty) || is_cached_table(ty))
}

/// Test whether a parameter is the canonical `DocString` shape.
///
/// True only when the identifier is literally `docstring` *and* the type is
/// `String`. As with [`should_classify_as_datatable`], a `docstring` parameter
/// of another type is not a match, leaving [`classify_docstring`] free to
/// reject it with a type-specific diagnostic.
pub(super) fn is_docstring_canonical(pat: &syn::Ident, ty: &syn::Type) -> bool {
    pat == "docstring" && is_string(ty)
}
