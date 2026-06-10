//! Diagnostics for first-party adapter detection falling back to the base
//! harness crate.
//!
//! When a `harness = ...` or `attributes = ...` path names a first-party
//! adapter type (for example `TokioHarness`) but cannot be matched to its
//! crate — a rename in `Cargo.toml`, a local re-export — the macros resolve
//! base API types through `rstest-bdd-harness` instead. This module makes
//! that fallback loud: a nightly `emit_warning!` diagnostic plus generated
//! tokens that produce a stable-toolchain `deprecated` warning carrying the
//! same guidance.

#[cfg(not(test))]
use proc_macro_error2::emit_warning;
use proc_macro2::{Span, TokenStream as TokenStream2};

use super::{CrateSpec, GPUI_HARNESS, TOKIO_HARNESS};
use super::{first_party_adapter_spec, path_last_ident_matches};

/// Find the first-party adapter spec whose adapter type name matches the
/// final segment of `adapter_path`, regardless of whether the full path was
/// recognized. Used to detect "looks first-party but unresolvable" paths.
fn fallback_candidate_spec(adapter_path: &syn::Path) -> Option<&'static CrateSpec> {
    [&TOKIO_HARNESS, &GPUI_HARNESS]
        .into_iter()
        .find(|spec| path_last_ident_matches(adapter_path, spec.adapter_type_names))
}

/// Render the diagnostic text for the first-party adapter fallback.
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
pub(super) fn emit_first_party_adapter_fallback_warning(adapter_path: &syn::Path) {
    let Some(spec) = fallback_candidate_spec(adapter_path) else {
        return;
    };
    let span = adapter_path
        .segments
        .last()
        .map_or_else(Span::call_site, |segment| segment.ident.span());
    let message = first_party_adapter_fallback_message(spec);
    emit_warning!(span, "{}", message);
}

#[cfg(test)]
pub(super) fn emit_first_party_adapter_fallback_warning(_: &syn::Path) {}

/// Build tokens that surface the first-party adapter fallback diagnostic on a
/// stable toolchain.
///
/// `proc_macro_error2`'s `emit_warning!` renders only on nightly (warnings are
/// ignored on stable), so the macro also emits a sibling `const _` block that
/// references a `#[deprecated]` unit struct whose note carries the fallback
/// message. On stable the user sees a `deprecated` warning pointing at the
/// supplied adapter path; under `deny(deprecated)` (as in the trybuild
/// coverage) it becomes a pinned error.
///
/// Returns empty tokens when the path resolves as a first-party adapter or
/// does not name a first-party adapter type at all, so canonical paths never
/// trigger the diagnostic.
pub(crate) fn first_party_adapter_fallback_warning_tokens(
    adapter_path: &syn::Path,
) -> TokenStream2 {
    if first_party_adapter_spec(adapter_path).is_some() {
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
            #[allow(dead_code, reason = "exists only to surface the deprecation warning")]
            fn __rstest_bdd_first_party_adapter_fallback_warning() {
                let _ = RstestBddFirstPartyAdapterFallback;
            }
        };
    }
}
