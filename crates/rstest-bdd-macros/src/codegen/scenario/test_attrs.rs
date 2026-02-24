//! Test-attribute policy resolution for generated scenario tests.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rstest_bdd_policy::resolve_test_attribute_hint_for_policy_path;

use super::{RuntimeMode, TestAttributeHint};

fn is_tokio_test_attr(attr: &syn::Attribute) -> bool {
    let mut segments = attr.path().segments.iter();
    let Some(first) = segments.next() else {
        return false;
    };
    let Some(second) = segments.next() else {
        return false;
    };
    segments.next().is_none() && first.ident == "tokio" && second.ident == "test"
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PolicyAttribute {
    Rstest,
    TokioCurrentThread,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResolvedAttributePolicy {
    Default,
    TokioCurrentThread,
}

impl ResolvedAttributePolicy {
    fn test_attributes(self) -> &'static [PolicyAttribute] {
        const DEFAULT: [PolicyAttribute; 1] = [PolicyAttribute::Rstest];
        const TOKIO: [PolicyAttribute; 2] =
            [PolicyAttribute::Rstest, PolicyAttribute::TokioCurrentThread];

        match self {
            Self::Default => &DEFAULT,
            Self::TokioCurrentThread => &TOKIO,
        }
    }
}

fn resolve_attribute_hint_from_policy_path(path: &syn::Path) -> Option<TestAttributeHint> {
    let segment_names: Vec<_> = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    let segment_refs: Vec<_> = segment_names.iter().map(String::as_str).collect();
    resolve_test_attribute_hint_for_policy_path(&segment_refs)
}

fn resolve_attribute_policy(
    runtime: RuntimeMode,
    attributes: Option<&syn::Path>,
) -> ResolvedAttributePolicy {
    let hint = attributes.map_or_else(
        || runtime.test_attribute_hint(),
        |path| {
            resolve_attribute_hint_from_policy_path(path).unwrap_or(TestAttributeHint::RstestOnly)
        },
    );
    match hint {
        TestAttributeHint::RstestOnly => ResolvedAttributePolicy::Default,
        TestAttributeHint::RstestWithTokioCurrentThread => {
            ResolvedAttributePolicy::TokioCurrentThread
        }
    }
}

fn render_policy_attribute(attribute: PolicyAttribute) -> TokenStream2 {
    match attribute {
        PolicyAttribute::Rstest => quote! { #[rstest::rstest] },
        PolicyAttribute::TokioCurrentThread => quote! {
            #[tokio::test(flavor = "current_thread")]
        },
    }
}

pub(super) fn generate_test_attrs(
    attrs: &[syn::Attribute],
    runtime: RuntimeMode,
    attributes: Option<&syn::Path>,
    is_async: bool,
) -> TokenStream2 {
    // Match only tokio::test to avoid false positives like #[test] or #[test_case].
    let has_tokio_test = attrs.iter().any(is_tokio_test_attr);
    let resolved_policy = resolve_attribute_policy(runtime, attributes);

    let generated_attrs: Vec<_> = resolved_policy
        .test_attributes()
        .iter()
        .copied()
        .filter_map(|attribute| match attribute {
            // Tokio test attributes require async test signatures.
            PolicyAttribute::TokioCurrentThread if !is_async || has_tokio_test => None,
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
