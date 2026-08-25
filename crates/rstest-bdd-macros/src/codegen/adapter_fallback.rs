//! Diagnostics for first-party adapter detection falling back to the base
//! harness crate.
//!
//! When a `harness = ...` or `attributes = ...` path names a first-party
//! adapter type (for example `TokioHarness`) but cannot be matched to its
//! crate — a rename in `Cargo.toml`, a local re-export — the macros resolve
//! base API types through `rstest-bdd-harness` instead. This module makes
//! that fallback loud: a native nightly diagnostic plus generated tokens that
//! produce a stable-toolchain `deprecated` warning carrying the same guidance.

use proc_macro2::{Span, TokenStream as TokenStream2};

use super::{CrateSpec, GPUI_HARNESS, TOKIO_HARNESS, path_last_ident_matches};

/// Find the first-party adapter spec whose adapter type name matches the
/// final segment of `adapter_path` when that adapter crate is resolvable.
/// Used to detect first-party re-exports without matching unrelated types that
/// merely share an adapter name.
pub(super) fn fallback_candidate(adapter_path: &syn::Path) -> Option<AdapterFallback> {
    [&TOKIO_HARNESS, &GPUI_HARNESS]
        .into_iter()
        .find_map(|spec| {
            (path_last_ident_matches(adapter_path, spec.adapter_type_names)
                && path_penultimate_ident_matches(adapter_path, spec.default_crate_name)
                && fallback_crate_is_resolvable(spec))
            .then(|| AdapterFallback {
                spec,
                span: adapter_path
                    .segments
                    .last()
                    .map_or_else(Span::call_site, |segment| segment.ident.span()),
            })
        })
}

#[cfg(not(test))]
fn fallback_crate_is_resolvable(spec: &CrateSpec) -> bool {
    super::try_resolve_crate_path(spec).is_some()
}

#[cfg(test)]
fn fallback_crate_is_resolvable(_: &CrateSpec) -> bool {
    // Unit tests exercise the syntactic decision independently of Cargo's
    // package-resolution environment.
    true
}

/// Metadata for one qualifying first-party adapter fallback.
#[derive(Clone, Copy)]
pub(super) struct AdapterFallback {
    spec: &'static CrateSpec,
    span: Span,
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
            "rstest-bdd could not identify this harness or attribute-policy path as a first-party \
             adapter; ",
            "falling back to `rstest-bdd-harness` for base harness API types. ",
            "Use the canonical crate-root path, ensure `{}` is directly resolvable as `{}`, ",
            "or add `rstest-bdd-harness` as a direct dev-dependency."
        ),
        spec.package_name, spec.default_crate_name
    )
}

#[cfg(not(test))]
#[cfg(rstest_bdd_nightly)]
pub(super) fn emit_first_party_adapter_fallback_warning(fallback: Option<&AdapterFallback>) {
    let Some(fallback) = fallback else {
        return;
    };
    let message = first_party_adapter_fallback_message(fallback.spec);
    proc_macro::Diagnostic::spanned(fallback.span.unwrap(), proc_macro::Level::Warning, message)
        .emit();
}

#[cfg(not(test))]
#[cfg(not(rstest_bdd_nightly))]
pub(super) fn emit_first_party_adapter_fallback_warning(_: Option<&AdapterFallback>) {}

#[cfg(test)]
pub(super) fn emit_first_party_adapter_fallback_warning(_: Option<&AdapterFallback>) {}

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
    fallback: Option<&AdapterFallback>,
) -> TokenStream2 {
    let Some(fallback) = fallback else {
        return TokenStream2::new();
    };
    let span = fallback.span;
    let message = first_party_adapter_fallback_message(fallback.spec);
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
pub(crate) fn first_party_adapter_fallback_warning_tokens(
    _: Option<&AdapterFallback>,
) -> TokenStream2 {
    TokenStream2::new()
}

/// Harness API resolutions shared by every scenario one macro expansion emits.
///
/// `scenarios!` generates a test per discovered scenario, so resolving the
/// supplied paths inside each scenario would repeat the fallback diagnostic
/// once per generated test. Resolving at the expansion boundary instead keeps
/// the contract at one diagnostic per distinct qualifying path, however many
/// scenarios the feature directory yields.
#[derive(Clone)]
pub(crate) struct SharedAdapterResolutions {
    pub(crate) harness: Option<super::HarnessApiResolution>,
    pub(crate) attributes: Option<super::HarnessApiResolution>,
}

impl SharedAdapterResolutions {
    /// Resolve both supplied paths without emitting anything.
    pub(crate) fn resolve(harness: Option<&syn::Path>, attributes: Option<&syn::Path>) -> Self {
        Self {
            harness: harness.map(super::resolve_harness_api),
            attributes: attributes.map(super::resolve_harness_api),
        }
    }

    /// Emit each qualifying fallback exactly once.
    ///
    /// Call this only from the expansion boundary that owns the resolutions.
    /// On nightly it emits through `proc_macro::Diagnostic`, so a second call
    /// would duplicate the warning. The returned tokens carry the stable
    /// deprecated-item form and must be spliced into the generated output.
    pub(crate) fn emit_diagnostics(&self) -> TokenStream2 {
        let harness = self
            .harness
            .as_ref()
            .map(super::first_party_adapter_fallback_diagnostic);
        let attributes = self
            .attributes
            .as_ref()
            .map(super::first_party_adapter_fallback_diagnostic);
        quote::quote! {
            #harness
            #attributes
        }
    }
}

#[cfg(test)]
mod tests {
    //! Regression tests for stable fallback-token selection.

    use proptest::prelude::*;

    use super::first_party_adapter_fallback_warning_tokens;

    fn recognized_adapter_type() -> impl Strategy<Value = &'static str> {
        prop_oneof![
            Just("TokioHarness"),
            Just("TokioAttributePolicy"),
            Just("GpuiHarness"),
            Just("GpuiAttributePolicy"),
        ]
    }

    fn canonical_adapter_crate() -> impl Strategy<Value = &'static str> {
        prop_oneof![
            Just("rstest_bdd_harness_tokio"),
            Just("rstest_bdd_harness_gpui"),
        ]
    }

    #[test]
    fn matching_custom_adapter_name_does_not_emit_warning_tokens() {
        let adapter_path = syn::parse_quote!(custom::TokioHarness);
        let fallback = super::fallback_candidate(&adapter_path);

        assert!(first_party_adapter_fallback_warning_tokens(fallback.as_ref()).is_empty());
    }

    #[test]
    fn custom_root_is_not_syntactic_first_party_evidence() {
        let adapter_path = syn::parse_quote!(custom::TokioHarness);

        assert!(!super::path_penultimate_ident_matches(
            &adapter_path,
            "rstest_bdd_harness_tokio"
        ));
    }

    #[test]
    fn canonical_tokio_and_gpui_paths_are_syntactic_first_party_evidence() {
        let cases = [
            (
                syn::parse_quote!(rstest_bdd_harness_tokio::TokioHarness),
                "rstest_bdd_harness_tokio",
            ),
            (
                syn::parse_quote!(rstest_bdd_harness_gpui::GpuiAttributePolicy),
                "rstest_bdd_harness_gpui",
            ),
        ];

        for (adapter_path, expected_crate) in cases {
            assert!(super::path_penultimate_ident_matches(
                &adapter_path,
                expected_crate
            ));
        }
    }

    #[test]
    fn fallback_metadata_retains_adapter_kind_and_terminal_span() {
        let adapter_path: syn::Path =
            syn::parse_quote!(alias::rstest_bdd_harness_tokio::TokioHarness);
        let Some(fallback) = super::fallback_candidate(&adapter_path) else {
            panic!("aliased canonical crate segment should qualify");
        };
        let Some(terminal) = adapter_path.segments.last() else {
            panic!("adapter path should have a terminal segment");
        };
        let terminal_span = terminal.ident.span();

        assert_eq!(fallback.spec.default_crate_name, "rstest_bdd_harness_tokio");
        assert_eq!(fallback.span.start(), terminal_span.start());
        assert_eq!(fallback.span.end(), terminal_span.end());
    }

    proptest! {
        #[test]
        fn penultimate_crate_match_is_exact_for_recognized_adapter_paths(
            adapter_type in recognized_adapter_type(),
            expected_crate in canonical_adapter_crate(),
            path_shape in 0_u8..3,
            prefix_segments in proptest::collection::vec("[a-z][a-z0-9_]{0,11}", 0..4),
            noncanonical_suffix in "[a-z][a-z0-9_]{0,11}",
        ) {
            let mut segments = if path_shape == 0 {
                Vec::new()
            } else {
                prefix_segments
                    .into_iter()
                    .map(|segment| format!("segment_{segment}"))
                    .collect()
            };
            if path_shape == 1 {
                segments.push(expected_crate.to_owned());
            } else if path_shape == 2 {
                segments.push(format!("custom_{noncanonical_suffix}"));
            }
            segments.push(adapter_type.to_owned());
            let path_text = segments.join("::");
            let adapter_path = syn::parse_str::<syn::Path>(&path_text)
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
            let expected_match = adapter_path
                .segments
                .iter()
                .zip(adapter_path.segments.iter().skip(1))
                .next_back()
                .is_some_and(|(penultimate, _terminal)| penultimate.ident == expected_crate);

            prop_assert_eq!(
                super::path_penultimate_ident_matches(&adapter_path, expected_crate),
                expected_match
            );
            prop_assert_eq!(expected_match, path_shape == 1);
        }
    }
}
