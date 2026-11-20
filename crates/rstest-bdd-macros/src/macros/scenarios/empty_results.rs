//! The compile error for a `scenarios!` expansion that generates no tests.
//!
//! An empty result almost always means a mistyped path or a feature whose
//! scenarios the macros cannot see, so the expansion fails loudly rather than
//! emitting an empty module that silently disables a suite.

use proc_macro2::TokenStream as TokenStream2;

use super::TagFilter;
use crate::utils::errors::error_to_tokens;

/// What an expansion emitted before the empty-results check runs.
#[derive(Clone, Copy)]
pub(super) struct ExpansionOutput<'a> {
    /// Generated scenario tests.
    pub(super) tests: &'a [TokenStream2],
    /// Adapter-resolution diagnostics, emitted once for the whole module.
    pub(super) fallback_diagnostics: &'a TokenStream2,
}

impl ExpansionOutput<'_> {
    /// Whether the expansion produced no tests and reported nothing else.
    ///
    /// A diagnostic that is already reported, such as an invalid harness
    /// adapter, explains the failure better than a second "no scenarios"
    /// error would.
    fn is_silent(self, errors: &[TokenStream2]) -> bool {
        self.tests.is_empty() && errors.is_empty() && self.fallback_diagnostics.is_empty()
    }
}

/// Report a compile error when an expansion generated no tests at all.
///
/// The error points at the path literal. When a tag filter is present, the
/// filter's more specific message is reported at the filter instead.
///
/// # Examples
///
/// ```rust,ignore
/// let dir = syn::LitStr::new("tests/features/empty", Span::call_site());
/// let mut errors = Vec::new();
/// let output = ExpansionOutput { tests: &[], fallback_diagnostics: &quote!() };
/// check_empty_results(output, &mut errors, None, &dir);
/// // errors now holds:
/// // compile_error!("no scenarios found in `tests/features/empty`")
/// ```
pub(super) fn check_empty_results(
    output: ExpansionOutput<'_>,
    errors: &mut Vec<TokenStream2>,
    tag_filter: Option<&TagFilter>,
    dir_lit: &syn::LitStr,
) {
    if !output.is_silent(errors) {
        return;
    }
    let err = tag_filter.map_or_else(
        || {
            syn::Error::new(
                dir_lit.span(),
                format!("no scenarios found in `{}`", dir_lit.value()),
            )
        },
        |filter| {
            syn::Error::new(
                filter.span,
                format!("no scenarios matched tag expression `{}`", filter.raw),
            )
        },
    );
    errors.push(error_to_tokens(&err));
}
