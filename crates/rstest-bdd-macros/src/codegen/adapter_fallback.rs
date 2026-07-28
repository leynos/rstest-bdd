//! Diagnostics for first-party adapter detection falling back to the base
//! harness crate.
//!
//! When a `harness = ...` or `attributes = ...` path names a first-party
//! adapter type (for example `TokioHarness`) but cannot be matched to its
//! crate — a rename in `Cargo.toml`, a local re-export — the macros resolve
//! base API types through `rstest-bdd-harness` instead. This module makes
//! that fallback loud: a native nightly diagnostic plus generated tokens that
//! produce a stable-toolchain `deprecated` warning carrying the same guidance.

#[cfg(any(not(test), not(rstest_bdd_nightly)))]
use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;

#[cfg(any(not(test), not(rstest_bdd_nightly)))]
use super::path_last_ident_matches;
#[cfg(any(not(test), not(rstest_bdd_nightly)))]
use super::{CrateSpec, GPUI_HARNESS, TOKIO_HARNESS};

/// Find the first-party adapter spec whose adapter type name matches the
/// final segment of `adapter_path` when that adapter crate is resolvable.
/// Used to detect first-party re-exports without matching unrelated types that
/// merely share an adapter name.
#[cfg(any(not(test), not(rstest_bdd_nightly)))]
fn fallback_candidate_spec(adapter_path: &syn::Path) -> Option<&'static CrateSpec> {
    [&TOKIO_HARNESS, &GPUI_HARNESS].into_iter().find(|spec| {
        path_last_ident_matches(adapter_path, spec.adapter_type_names)
            && path_penultimate_ident_matches(adapter_path, spec.default_crate_name)
            && super::try_resolve_crate_path(spec).is_some()
    })
}

fn path_penultimate_ident_matches(path: &syn::Path, expected: &str) -> bool {
    let mut segments = path.segments.iter().rev();
    let _adapter_type = segments.next();
    segments
        .next()
        .is_some_and(|segment| segment.ident == expected)
}

/// Render the diagnostic text for the first-party adapter fallback.
#[cfg(any(not(test), not(rstest_bdd_nightly)))]
fn first_party_adapter_fallback_message(spec: &CrateSpec) -> String {
    format!(
        concat!(
            "rstest-bdd could not identify this harness or attribute-policy path as a first-party adapter; ",
            "falling back to `rstest-bdd-harness` for base harness API types. ",
            "Use the canonical crate-root path, ensure `{}` is directly resolvable as `{}`, ",
            "or add `rstest-bdd-harness` as a direct dev-dependency."
        ),
        spec.package_name, spec.default_crate_name
    )
}

#[cfg(not(test))]
#[cfg(rstest_bdd_nightly)]
pub(super) fn emit_first_party_adapter_fallback_warning(adapter_path: &syn::Path) {
    let Some(spec) = fallback_candidate_spec(adapter_path) else {
        return;
    };
    let span = adapter_path
        .segments
        .last()
        .map_or_else(Span::call_site, |segment| segment.ident.span());
    let message = first_party_adapter_fallback_message(spec);
    proc_macro::Diagnostic::spanned(span.unwrap(), proc_macro::Level::Warning, message).emit();
}

#[cfg(not(test))]
#[cfg(not(rstest_bdd_nightly))]
pub(super) fn emit_first_party_adapter_fallback_warning(_: &syn::Path) {}

#[cfg(test)]
pub(super) fn emit_first_party_adapter_fallback_warning(_: &syn::Path) {}

/// Build tokens that surface the first-party adapter fallback diagnostic on a
/// stable toolchain.
///
/// Stable procedural macros cannot emit native warning diagnostics, so the
/// macro emits a sibling `const _` block that references a `#[deprecated]`
/// unit struct whose note carries the fallback message. The user sees a
/// `deprecated` warning pointing at the supplied adapter path; under
/// `deny(deprecated)` (as in the trybuild coverage) it becomes a pinned error.
///
/// Returns empty tokens when the path resolves as a first-party adapter or
/// does not name a first-party adapter type at all, so canonical paths never
/// trigger the diagnostic.
#[cfg(not(rstest_bdd_nightly))]
pub(crate) fn first_party_adapter_fallback_warning_tokens(
    adapter_path: &syn::Path,
) -> TokenStream2 {
    if super::first_party_adapter_spec(adapter_path).is_some() {
        return TokenStream2::new();
    }
    let Some(spec) = fallback_candidate_spec(adapter_path) else {
        return TokenStream2::new();
    };
    let span = adapter_path
        .segments
        .last()
        .map_or_else(Span::call_site, |segment| segment.ident.span());
    let message = first_party_adapter_fallback_message(spec);
    quote::quote_spanned! {span=>
        const _: () = {
            #[deprecated(note = #message)]
            struct RstestBddFirstPartyAdapterFallback;
            let _ = RstestBddFirstPartyAdapterFallback;
        };
    }
}

/// Nightly receives the same diagnostic through `proc_macro::Diagnostic`.
#[cfg(rstest_bdd_nightly)]
pub(crate) fn first_party_adapter_fallback_warning_tokens(_: &syn::Path) -> TokenStream2 {
    TokenStream2::new()
}

#[cfg(test)]
mod tests {
    //! Regression tests for stable fallback-token selection.

    use super::first_party_adapter_fallback_warning_tokens;

    #[test]
    fn matching_custom_adapter_name_does_not_emit_warning_tokens() {
        let adapter_path = syn::parse_quote!(custom::TokioHarness);

        assert!(first_party_adapter_fallback_warning_tokens(&adapter_path).is_empty());
    }

    #[test]
    fn custom_root_is_not_syntactic_first_party_evidence() {
        let adapter_path = syn::parse_quote!(custom::TokioHarness);

        assert!(!super::path_penultimate_ident_matches(
            &adapter_path,
            "rstest_bdd_harness_tokio"
        ));
    }
}
