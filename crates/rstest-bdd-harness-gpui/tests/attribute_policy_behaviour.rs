//! Behavioural tests for GPUI attribute policy output.
#![cfg(feature = "native-gpui-tests")]

use rstest_bdd_harness::{AttributePolicy, TestAttribute};
use rstest_bdd_harness_gpui::GpuiAttributePolicy;

#[test]
fn gpui_policy_emits_rstest_and_gpui_test_attributes() {
    let attributes = GpuiAttributePolicy::test_attributes();
    assert_eq!(
        attributes,
        [
            TestAttribute::new("rstest::rstest"),
            TestAttribute::new("gpui::test"),
        ]
    );
    let rendered: Vec<_> = attributes
        .iter()
        .copied()
        .map(TestAttribute::render)
        .collect();
    assert_eq!(rendered, vec!["#[rstest::rstest]", "#[gpui::test]"]);
}

#[test]
fn gpui_policy_attributes_preserve_order() {
    let attributes = GpuiAttributePolicy::test_attributes();
    assert_eq!(attributes.first().map(|a| a.path()), Some("rstest::rstest"));
    assert_eq!(attributes.get(1).map(|a| a.path()), Some("gpui::test"));
}
