//! Test-attribute policy resolution for generated scenario tests.
//!
//! This module implements the ADR-008 precedence chain that selects which
//! framework test attributes are emitted alongside `#[rstest::rstest]` for
//! each generated test function:
//!
//! 1. An explicit `attributes = …` path supplied to the `#[scenario]` or
//!    `#[scenarios]` macro takes highest precedence.
//! 2. A `harness = …` path is consulted next; first-party adapter types
//!    (`TokioHarness`, `GpuiHarness`) are recognized via
//!    [`crate::codegen::first_party_adapter_attribute_hint`].
//! 3. The `RuntimeMode` derived from `runtime = …` or the macro's defaults
//!    provides the final fallback.
//!
//! The public surface is [`generate_test_attrs`] and [`TestAttrPolicy`].
//! Existing user attributes (`#[tokio::test]`, `#[gpui::test]`) are
//! detected so that generated output does not duplicate them.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rstest_bdd_policy::{
    resolve_test_attribute_hint_for_harness_path, resolve_test_attribute_hint_for_policy_path,
};

use crate::codegen::first_party_adapter_attribute_hint;

use super::{RuntimeMode, TestAttributeHint};

/// Returns `true` when `attr` is exactly `<crate_name>::<fn_name>`.
fn is_two_segment_attr(attr: &syn::Attribute, crate_name: &str, fn_name: &str) -> bool {
    let mut segments = attr.path().segments.iter();
    let Some(first) = segments.next() else {
        return false;
    };
    let Some(second) = segments.next() else {
        return false;
    };
    segments.next().is_none() && first.ident == crate_name && second.ident == fn_name
}

fn is_tokio_test_attr(attr: &syn::Attribute) -> bool {
    is_two_segment_attr(attr, "tokio", "test")
}

fn is_gpui_test_attr(attr: &syn::Attribute) -> bool {
    is_two_segment_attr(attr, "gpui", "test")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PolicyAttribute {
    Rstest,
    TokioCurrentThread,
    GpuiTest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResolvedAttributePolicy {
    Default,
    TokioCurrentThread,
    Gpui,
}

impl ResolvedAttributePolicy {
    fn test_attributes(self) -> &'static [PolicyAttribute] {
        const DEFAULT: [PolicyAttribute; 1] = [PolicyAttribute::Rstest];
        const TOKIO: [PolicyAttribute; 2] =
            [PolicyAttribute::Rstest, PolicyAttribute::TokioCurrentThread];
        const GPUI: [PolicyAttribute; 2] = [PolicyAttribute::Rstest, PolicyAttribute::GpuiTest];

        match self {
            Self::Default => &DEFAULT,
            Self::TokioCurrentThread => &TOKIO,
            Self::Gpui => &GPUI,
        }
    }
}

fn resolve_attribute_hint_from_policy_path(path: &syn::Path) -> Option<TestAttributeHint> {
    resolve_attribute_hint_from_path(path, resolve_test_attribute_hint_for_policy_path)
        .or_else(|| first_party_adapter_attribute_hint(path))
}

fn resolve_attribute_hint_from_harness_path(path: &syn::Path) -> Option<TestAttributeHint> {
    resolve_attribute_hint_from_path(path, resolve_test_attribute_hint_for_harness_path)
        .or_else(|| first_party_adapter_attribute_hint(path))
}

fn resolve_attribute_hint_from_path(
    path: &syn::Path,
    resolver: fn(&[&str]) -> Option<TestAttributeHint>,
) -> Option<TestAttributeHint> {
    let segment_names: Vec<_> = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    let segment_refs: Vec<_> = segment_names.iter().map(String::as_str).collect();
    resolver(&segment_refs)
}

/// Policy-resolution inputs bundled for ADR-008 precedence.
pub(super) struct TestAttrPolicy<'a> {
    /// Attribute-resolution fallback `RuntimeMode` used when no higher-priority
    /// harness or explicit attribute policy path resolves a `TestAttributeHint`.
    pub(super) runtime: RuntimeMode,
    /// User-supplied harness type path, such as
    /// `rstest_bdd_harness_tokio::TokioHarness`.
    pub(super) harness: Option<&'a syn::Path>,
    /// Explicit attribute policy path supplied by the macro caller; this has
    /// the highest ADR-008 precedence when present.
    pub(super) attributes: Option<&'a syn::Path>,
}

fn resolve_attribute_policy(policy: &TestAttrPolicy<'_>) -> ResolvedAttributePolicy {
    let hint = policy.attributes.map_or_else(
        || {
            policy.harness.map_or_else(
                || policy.runtime.test_attribute_hint(),
                |path| {
                    resolve_attribute_hint_from_harness_path(path)
                        .unwrap_or_else(|| policy.runtime.test_attribute_hint())
                },
            )
        },
        |path| {
            resolve_attribute_hint_from_policy_path(path).unwrap_or(TestAttributeHint::RstestOnly)
        },
    );
    match hint {
        TestAttributeHint::RstestOnly => ResolvedAttributePolicy::Default,
        TestAttributeHint::RstestWithTokioCurrentThread => {
            ResolvedAttributePolicy::TokioCurrentThread
        }
        TestAttributeHint::RstestWithGpuiTest => ResolvedAttributePolicy::Gpui,
    }
}

fn render_policy_attribute(attribute: PolicyAttribute) -> TokenStream2 {
    match attribute {
        PolicyAttribute::Rstest => quote! { #[rstest::rstest] },
        PolicyAttribute::TokioCurrentThread => quote! {
            #[tokio::test(flavor = "current_thread")]
        },
        PolicyAttribute::GpuiTest => quote! { #[gpui::test] },
    }
}

/// Generates framework test attributes according to the ADR-008 policy order.
///
/// The emitted `TokenStream2` always includes `#[rstest::rstest]`, then layers
/// any Tokio or GPUI test attribute selected by explicit `attributes`,
/// first-party `harness`, or `RuntimeMode` fallback precedence. Existing user
/// attributes are inspected so generated output does not duplicate
/// `#[tokio::test]` or `#[gpui::test]`, and Tokio attributes are omitted for
/// synchronous test signatures.
///
/// # Examples
///
/// ```rust,ignore
/// let tokens = generate_test_attrs(
///     &[],
///     &TestAttrPolicy {
///         runtime: RuntimeMode::TokioCurrentThread,
///         harness: None,
///         attributes: None,
///     },
///     true,
/// );
///
/// assert!(tokens.to_string().contains("tokio :: test"));
/// ```
pub(super) fn generate_test_attrs(
    attrs: &[syn::Attribute],
    policy: &TestAttrPolicy<'_>,
    is_async: bool,
) -> TokenStream2 {
    // Match only tokio::test to avoid false positives like #[test] or #[test_case].
    let has_tokio_test = attrs.iter().any(is_tokio_test_attr);
    let has_gpui_test = attrs.iter().any(is_gpui_test_attr);
    let resolved_policy = resolve_attribute_policy(policy);

    let generated_attrs: Vec<_> = resolved_policy
        .test_attributes()
        .iter()
        .copied()
        .filter_map(|attribute| match attribute {
            // Tokio test attributes require async test signatures.
            PolicyAttribute::TokioCurrentThread if !is_async || has_tokio_test => None,
            PolicyAttribute::GpuiTest if has_gpui_test => None,
            _ => Some(render_policy_attribute(attribute)),
        })
        .collect();

    if generated_attrs.is_empty() {
        quote! { #[rstest::rstest] }
    } else {
        quote! {
            #(#generated_attrs)*
        }
    }
}
