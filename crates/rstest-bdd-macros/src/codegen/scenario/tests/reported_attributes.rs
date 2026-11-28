//! Representation refinement: emitted attributes agree with the model.
//!
//! The model's `selected_hint` is only useful if the code generator emits the
//! attributes it implies. This table compares the real
//! [`generate_test_attrs`] output against the model for one concrete witness
//! path per slot class, across both runtime modes and both signature shapes, so
//! a refactor that changed emitted attributes would fail here rather than in a
//! consumer's build.

use proc_macro2::TokenStream as TokenStream2;
use rstest_bdd_policy::TestAttributeHint;

use super::{RuntimeMode, TestAttrPolicy, generate_test_attrs};
use crate::codegen::{
    Framework,
    SlotClass,
    classify_harness_path,
    classify_policy_path,
    default_resolver,
    selected_hint,
};

/// Every slot class the classifier can produce.
const CLASSES: [SlotClass; 7] = [
    SlotClass::Absent,
    SlotClass::Recognized(TestAttributeHint::RstestOnly),
    SlotClass::Recognized(TestAttributeHint::RstestWithTokioCurrentThread),
    SlotClass::Recognized(TestAttributeHint::RstestWithGpuiTest),
    SlotClass::Evidenced(Framework::Tokio),
    SlotClass::Evidenced(Framework::Gpui),
    SlotClass::Foreign,
];

/// A witness path that classifies to `class` in the harness slot.
fn harness_witness(class: SlotClass) -> Option<syn::Path> {
    match class {
        SlotClass::Absent => None,
        SlotClass::Recognized(TestAttributeHint::RstestOnly) => {
            Some(syn::parse_quote!(rstest_bdd_harness::StdHarness))
        }
        SlotClass::Recognized(TestAttributeHint::RstestWithTokioCurrentThread) => {
            Some(syn::parse_quote!(rstest_bdd_harness_tokio::TokioHarness))
        }
        SlotClass::Recognized(TestAttributeHint::RstestWithGpuiTest) => {
            Some(syn::parse_quote!(rstest_bdd_harness_gpui::GpuiHarness))
        }
        SlotClass::Evidenced(Framework::Tokio) => Some(syn::parse_quote!(
            alias::rstest_bdd_harness_tokio::TokioHarness
        )),
        SlotClass::Evidenced(Framework::Gpui) => Some(syn::parse_quote!(
            alias::rstest_bdd_harness_gpui::GpuiHarness
        )),
        SlotClass::Foreign => Some(syn::parse_quote!(custom::TokioHarness)),
    }
}

/// A witness path that classifies to `class` in the attributes slot.
fn attributes_witness(class: SlotClass) -> Option<syn::Path> {
    match class {
        SlotClass::Absent => None,
        SlotClass::Recognized(TestAttributeHint::RstestOnly) => Some(syn::parse_quote!(
            rstest_bdd_harness::DefaultAttributePolicy
        )),
        SlotClass::Recognized(TestAttributeHint::RstestWithTokioCurrentThread) => Some(
            syn::parse_quote!(rstest_bdd_harness_tokio::TokioAttributePolicy),
        ),
        SlotClass::Recognized(TestAttributeHint::RstestWithGpuiTest) => Some(syn::parse_quote!(
            rstest_bdd_harness_gpui::GpuiAttributePolicy
        )),
        SlotClass::Evidenced(Framework::Tokio) => Some(syn::parse_quote!(
            alias::rstest_bdd_harness_tokio::TokioAttributePolicy
        )),
        SlotClass::Evidenced(Framework::Gpui) => Some(syn::parse_quote!(
            alias::rstest_bdd_harness_gpui::GpuiAttributePolicy
        )),
        SlotClass::Foreign => Some(syn::parse_quote!(custom::Policy)),
    }
}

#[test]
fn every_witness_classifies_as_intended() {
    for class in CLASSES {
        if let Some(path) = harness_witness(class) {
            assert_eq!(
                classify_harness_path(&path, default_resolver()),
                class,
                "the harness witness must classify as intended"
            );
        }
        if let Some(path) = attributes_witness(class) {
            assert_eq!(
                classify_policy_path(&path, default_resolver()),
                class,
                "the attributes witness must classify as intended"
            );
        }
    }

    for class in CLASSES {
        let is_absent = class == SlotClass::Absent;
        assert_eq!(
            harness_witness(class).is_some(),
            !is_absent,
            "every present harness class needs a witness"
        );
        assert_eq!(
            attributes_witness(class).is_some(),
            !is_absent,
            "every present attributes class needs a witness"
        );
    }
}

#[test]
fn generated_attributes_match_the_selected_hint() {
    for harness_class in CLASSES {
        for attributes_class in CLASSES {
            assert_generated_attributes(harness_class, attributes_class);
        }
    }
}

/// Compare one input pair's generated attributes with the model.
fn assert_generated_attributes(harness_class: SlotClass, attributes_class: SlotClass) {
    let harness = harness_witness(harness_class);
    let attributes = attributes_witness(attributes_class);
    for runtime in [RuntimeMode::Sync, RuntimeMode::TokioCurrentThread] {
        for is_async in [false, true] {
            let hint = selected_hint(harness_class, attributes_class, runtime);
            let tokens = generate_test_attrs(
                &[],
                &TestAttrPolicy {
                    runtime,
                    harness: harness.as_ref(),
                    attributes: attributes.as_ref(),
                },
                is_async,
            );
            let expect_tokio = hint == TestAttributeHint::RstestWithTokioCurrentThread && is_async;
            let expect_gpui = hint == TestAttributeHint::RstestWithGpuiTest;
            let context = format!(
                "{harness_class:?} / {attributes_class:?}, runtime {runtime:?}, async {is_async}"
            );

            assert_eq!(
                has_attribute(&tokens, "tokio :: test"),
                expect_tokio,
                "tokio attribute mismatch for {context}: {tokens}"
            );
            assert_eq!(
                has_attribute(&tokens, "gpui :: test"),
                expect_gpui,
                "gpui attribute mismatch for {context}: {tokens}"
            );
            assert!(
                has_attribute(&tokens, "rstest :: rstest"),
                "rstest is unconditional for {context}: {tokens}"
            );
        }
    }
}

/// Whether the generated token stream carries an attribute path.
fn has_attribute(tokens: &TokenStream2, attribute: &str) -> bool {
    tokens.to_string().contains(attribute)
}
