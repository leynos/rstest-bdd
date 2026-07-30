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
    let mut cur = ty;
    for (i, &name) in seq.iter().enumerate() {
        let Some(segment) = final_path_segment(cur) else {
            return false;
        };
        if segment.ident != name {
            return false;
        }
        match generic_descent(segment) {
            Some(Descent::Into(inner)) => cur = inner,
            // A terminal segment satisfies `seq` only by being its last entry;
            // names still outstanding mean the chain ran out of nesting early.
            Some(Descent::Terminal) => return i + 1 == seq.len(),
            None => return false,
        }
    }
    true
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

/// Where the walk goes after a segment's name has matched.
enum Descent<'a> {
    /// The segment carries a first generic type argument to continue into.
    Into(&'a syn::Type),
    /// The segment has no generic argument to follow, so it can only be a leaf.
    Terminal,
}

/// Decide whether to descend into `segment`'s first generic argument.
///
/// Returns `Some(Descent::Into(_))` when the segment has a non-empty
/// angle-bracketed argument list whose first entry is a type,
/// `Some(Descent::Terminal)` when there is no argument list to follow (a bare
/// identifier, or a parenthesized `Fn(..)` form), and `None` when arguments are
/// present but the first is not a type — a lifetime or const, which the walk
/// cannot follow and treats as a mismatch rather than a leaf.
fn generic_descent(segment: &syn::PathSegment) -> Option<Descent<'_>> {
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Some(Descent::Terminal);
    };
    if args.args.is_empty() {
        return Some(Descent::Terminal);
    }
    match args.args.first() {
        Some(syn::GenericArgument::Type(inner)) => Some(Descent::Into(inner)),
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    //! Shape-predicate tests, pinning the walk's interior branches.
    //!
    //! Before these, coverage reached `is_type_seq` only indirectly through the
    //! `DataTable`/`DocString` classifiers, which exercised a fully-formed
    //! correct chain and a first-segment name mismatch. The early-termination
    //! guard, a wrong leaf at depth, a non-path type at depth, and a
    //! non-type first generic argument were all unpinned.

    use super::*;
    use rstest::rstest;
    use syn::parse_quote;

    #[rstest]
    // Correct chains, bare and module-qualified: only the last segment counts.
    #[case::string(parse_quote!(String), &["String"][..], true)]
    #[case::qualified_string(parse_quote!(std::string::String), &["String"][..], true)]
    #[case::datatable(parse_quote!(Vec<Vec<String>>), &["Vec", "Vec", "String"][..], true)]
    #[case::qualified_datatable(
        parse_quote!(std::vec::Vec<std::vec::Vec<std::string::String>>),
        &["Vec", "Vec", "String"][..],
        true
    )]
    // Name mismatch at the first segment, and at depth.
    #[case::wrong_root(parse_quote!(Option<String>), &["Vec", "String"][..], false)]
    #[case::wrong_leaf_at_depth(parse_quote!(Vec<Vec<u32>>), &["Vec", "Vec", "String"][..], false)]
    // Terminal segment reached with names still outstanding.
    #[case::chain_too_short(parse_quote!(Vec<String>), &["Vec", "Vec", "String"][..], false)]
    #[case::bare_root(parse_quote!(Vec), &["Vec", "String"][..], false)]
    // A longer type than `seq` asks for still matches: the walk stops early.
    #[case::seq_shorter_than_type(parse_quote!(Vec<Vec<String>>), &["Vec"][..], true)]
    // Non-path types are never looked through, at the root or at depth.
    #[case::reference(parse_quote!(&str), &["str"][..], false)]
    #[case::tuple(parse_quote!((String,)), &["String"][..], false)]
    #[case::reference_at_depth(parse_quote!(Vec<&str>), &["Vec", "str"][..], false)]
    // Arguments present, but the first is a lifetime rather than a type.
    #[case::lifetime_first(parse_quote!(Cow<'a, str>), &["Cow", "str"][..], false)]
    fn is_type_seq_matches_expected_shapes(
        #[case] ty: syn::Type,
        #[case] seq: &[&str],
        #[case] expected: bool,
    ) {
        assert_eq!(is_type_seq(&ty, seq), expected);
    }

    #[rstest]
    #[case::canonical(parse_quote!(String), true)]
    #[case::qualified(parse_quote!(std::string::String), true)]
    #[case::wrong(parse_quote!(usize), false)]
    fn is_string_accepts_only_string(#[case] ty: syn::Type, #[case] expected: bool) {
        assert_eq!(is_string(&ty), expected);
    }

    #[rstest]
    #[case::canonical(parse_quote!(Vec<Vec<String>>), true)]
    #[case::wrong_leaf(parse_quote!(Vec<Vec<u32>>), false)]
    #[case::too_shallow(parse_quote!(Vec<String>), false)]
    fn is_datatable_accepts_only_nested_string_vectors(
        #[case] ty: syn::Type,
        #[case] expected: bool,
    ) {
        assert_eq!(is_datatable(&ty), expected);
    }

    #[rstest]
    // A `docstring` parameter matches only at type `String`; any other name is
    // not this classifier's concern regardless of type.
    #[case::canonical("docstring", parse_quote!(String), true)]
    #[case::wrong_type("docstring", parse_quote!(usize), false)]
    #[case::other_name("body", parse_quote!(String), false)]
    fn is_docstring_canonical_requires_name_and_type(
        #[case] name: &str,
        #[case] ty: syn::Type,
        #[case] expected: bool,
    ) {
        let pat = syn::Ident::new(name, proc_macro2::Span::call_site());
        assert_eq!(is_docstring_canonical(&pat, &ty), expected);
    }
}
