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
/// Descends through each type's first generic argument once per entry in `seq`,
/// comparing each against the *final* path segment, so both `String` and
/// `std::string::String` match `["String"]`. `["Vec", "Vec", "String"]` matches
/// `Vec<Vec<String>>`, but not a chain with a different leaf, nor one that runs
/// out of nesting before the names are exhausted.
///
/// The final name is largely a leaf check: once it matches, the generic
/// arguments that segment carries are not inspected, so `String<u8>` matches
/// `["String"]` and `Vec<Vec<String>>` matches `["Vec"]`. Callers rely on this
/// for the `CachedTable` and `String` shapes, which are matched by name alone.
/// The one exception is a first generic argument that is a lifetime or const:
/// `Vec<'a, String>` does *not* match `["Vec"]`. That is inherited behaviour,
/// preserved deliberately rather than by accident — see
/// [`has_unfollowable_generic`].
///
/// Returns `false` for any non-path type (references, tuples, slices) rather
/// than looking through it, and `true` for an empty `seq`, which requires
/// nothing.
pub(super) fn is_type_seq(ty: &syn::Type, seq: &[&str]) -> bool {
    let [name, rest @ ..] = seq else {
        // Nothing left to require: whatever `ty` is, it satisfies the sequence.
        return true;
    };
    let Some(segment) = final_path_segment(ty) else {
        return false;
    };
    if segment.ident != name {
        return false;
    }
    if rest.is_empty() {
        // The final name matched, and its own generic arguments are not
        // inspected — `String<u8>` matches `["String"]`. The one exception is a
        // first argument the walk could never follow, which the original
        // matcher rejected even here; see `has_unfollowable_generic`.
        return !has_unfollowable_generic(segment);
    }
    // Names remain, so the chain must nest one level further.
    first_generic_type(segment).is_some_and(|inner| is_type_seq(inner, rest))
}

/// Borrow the final segment of `ty`'s path, or `None` if it is not a path type.
///
/// Only the last segment carries the name being matched, so `String` and
/// `std::string::String` are indistinguishable here — module qualification is
/// ignored by design. A non-path type (reference, tuple, slice) yields `None`
/// rather than being looked through.
fn final_path_segment(ty: &syn::Type) -> Option<&syn::PathSegment> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    path.path.segments.last()
}

/// Borrow the first generic argument of `segment`, if it is a type.
///
/// Yields `None` in every case the walk cannot follow: a segment with no
/// angle-bracketed arguments (a bare identifier, or a parenthesized `Fn(..)`
/// form), an empty argument list, or a first argument that is a lifetime or
/// const rather than a type. Only the *first* argument is considered, so
/// `Result<T, E>` descends into `T`.
fn first_generic_type(segment: &syn::PathSegment) -> Option<&syn::Type> {
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    match args.args.first() {
        Some(syn::GenericArgument::Type(inner)) => Some(inner),
        _ => None,
    }
}

/// Whether `segment`'s first generic argument is present but not a type.
///
/// True only for an angle-bracketed list whose first entry is a lifetime or
/// const — something the walk can never follow. A segment with no arguments, or
/// an empty list, is not "unfollowable"; it is simply a leaf.
///
/// This exists to preserve a corner of the original matcher: it decided whether
/// it could descend *before* asking whether any names remained, so a segment
/// like `Vec<'a, String>` was rejected against `["Vec"]` even though the name
/// matched and nothing further was required. Folding that check into the leaf
/// case keeps `is_type_seq` behaviourally identical while it reads top-down.
fn has_unfollowable_generic(segment: &syn::PathSegment) -> bool {
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return false;
    };
    matches!(args.args.first(), Some(arg) if !matches!(arg, syn::GenericArgument::Type(_)))
}

/// Test whether `ty` is `String`, the only type a `DocString` parameter accepts.
pub(super) fn is_string(ty: &syn::Type) -> bool { is_type_seq(ty, &["String"]) }

/// Test whether `ty` is the raw `Vec<Vec<String>>` table representation.
pub(super) fn is_datatable(ty: &syn::Type) -> bool { is_type_seq(ty, &["Vec", "Vec", "String"]) }

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

#[cfg(test)]
mod tests {
    //! Shape-predicate tests, pinning the walk's interior branches.
    //!
    //! Before these, coverage reached `is_type_seq` only indirectly through the
    //! `DataTable`/`DocString` classifiers, which exercised a fully-formed
    //! correct chain and a first-segment name mismatch. The early-termination
    //! guard, a wrong leaf at depth, a non-path type at depth, and a
    //! non-type first generic argument were all unpinned.

    use rstest::rstest;

    use super::*;

    /// Parse a type from source, failing loudly when a test input is malformed.
    ///
    /// Types are parsed directly rather than driven through macro expansion, so
    /// a failure here is a defective test case, not a predicate bug.
    fn ty(source: &str) -> syn::Type {
        match syn::parse_str::<syn::Type>(source) {
            Ok(parsed) => parsed,
            Err(err) => panic!("test input `{source}` did not parse as a type: {err}"),
        }
    }

    #[rstest]
    // Correct chains, bare and module-qualified: only the last segment counts.
    #[case::string("String", &["String"][..], true)]
    #[case::qualified_string("std::string::String", &["String"][..], true)]
    #[case::datatable("Vec<Vec<String>>", &["Vec", "Vec", "String"][..], true)]
    #[case::qualified_datatable(
        "std::vec::Vec<std::vec::Vec<std::string::String>>",
        &["Vec", "Vec", "String"][..],
        true
    )]
    // Name mismatch at the first segment, and at depth.
    #[case::wrong_root("Option<String>", &["Vec", "String"][..], false)]
    #[case::wrong_leaf_at_depth("Vec<Vec<u32>>", &["Vec", "Vec", "String"][..], false)]
    // Nesting runs out before the names do.
    #[case::chain_too_short("Vec<String>", &["Vec", "Vec", "String"][..], false)]
    #[case::bare_root("Vec", &["Vec", "String"][..], false)]
    // Leaf contract: once the final name matches, its own generic arguments are
    // not inspected. Both cases must stay `true`.
    #[case::seq_shorter_than_type("Vec<Vec<String>>", &["Vec"][..], true)]
    #[case::leaf_ignores_its_generics("String<u8>", &["String"][..], true)]
    // Non-path types are never looked through, at the root or at depth.
    #[case::reference("&String", &["String"][..], false)]
    #[case::tuple("(String,)", &["String"][..], false)]
    #[case::reference_at_depth("Vec<&str>", &["Vec", "str"][..], false)]
    // Arguments present, but the first is a lifetime rather than a type, so the
    // walk cannot descend when a further name still requires it.
    #[case::lifetime_first_blocks_descent("Wrapper<'a>", &["Wrapper", "String"][..], false)]
    #[case::lifetime_first_with_type("Cow<'a, str>", &["Cow", "str"][..], false)]
    // Inherited corner: an unfollowable first argument is rejected even at the
    // final name, where the leaf rule would otherwise accept it.
    #[case::lifetime_first_at_leaf("Vec<'a, String>", &["Vec"][..], false)]
    #[case::const_first_at_leaf("Wrapper<3>", &["Wrapper"][..], false)]
    // An empty sequence requires nothing.
    #[case::empty_seq("String", &[] as &[&str], true)]
    #[case::empty_seq_non_path("&String", &[] as &[&str], true)]
    fn is_type_seq_matches_expected_shapes(
        #[case] source: &str,
        #[case] seq: &[&str],
        #[case] expected: bool,
    ) {
        assert_eq!(is_type_seq(&ty(source), seq), expected);
    }

    #[rstest]
    #[case::canonical("String", true)]
    #[case::qualified("std::string::String", true)]
    #[case::wrong("usize", false)]
    fn is_string_accepts_only_string(#[case] source: &str, #[case] expected: bool) {
        assert_eq!(is_string(&ty(source)), expected);
    }

    #[rstest]
    #[case::canonical("Vec<Vec<String>>", true)]
    #[case::wrong_leaf("Vec<Vec<u32>>", false)]
    #[case::too_shallow("Vec<String>", false)]
    fn is_datatable_accepts_only_nested_string_vectors(
        #[case] source: &str,
        #[case] expected: bool,
    ) {
        assert_eq!(is_datatable(&ty(source)), expected);
    }

    #[rstest]
    // A `docstring` parameter matches only at type `String`; any other name is
    // not this classifier's concern regardless of type.
    #[case::canonical("docstring", "String", true)]
    #[case::wrong_type("docstring", "usize", false)]
    #[case::other_name("body", "String", false)]
    fn is_docstring_canonical_requires_name_and_type(
        #[case] name: &str,
        #[case] source: &str,
        #[case] expected: bool,
    ) {
        let pat = syn::Ident::new(name, proc_macro2::Span::call_site());
        assert_eq!(is_docstring_canonical(&pat, &ty(source)), expected);
    }
}
