//! Test-attribute policy resolution for generated scenario tests.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rstest_bdd_policy::{
    resolve_test_attribute_hint_for_harness_path, resolve_test_attribute_hint_for_policy_path,
};

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
}

fn resolve_attribute_hint_from_harness_path(path: &syn::Path) -> Option<TestAttributeHint> {
    resolve_attribute_hint_from_path(path, resolve_test_attribute_hint_for_harness_path)
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

fn resolve_attribute_policy(
    runtime: RuntimeMode,
    harness: Option<&syn::Path>,
    attributes: Option<&syn::Path>,
) -> ResolvedAttributePolicy {
    let hint = attributes.map_or_else(
        || {
            harness.map_or_else(
                || runtime.test_attribute_hint(),
                |path| {
                    resolve_attribute_hint_from_harness_path(path)
                        .unwrap_or_else(|| runtime.test_attribute_hint())
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

pub(super) fn generate_test_attrs(
    attrs: &[syn::Attribute],
    runtime: RuntimeMode,
    harness: Option<&syn::Path>,
    attributes: Option<&syn::Path>,
    is_async: bool,
) -> TokenStream2 {
    // Match only tokio::test to avoid false positives like #[test] or #[test_case].
    let has_tokio_test = attrs.iter().any(is_tokio_test_attr);
    let has_gpui_test = attrs.iter().any(is_gpui_test_attr);
    let resolved_policy = resolve_attribute_policy(runtime, harness, attributes);

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
