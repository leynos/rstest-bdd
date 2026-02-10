//! Behavioural tests for attribute policy output.

use rstest_bdd_harness::{AttributePolicy, DefaultAttributePolicy, TestAttribute};

#[test]
fn default_policy_emits_rstest_only_attribute() {
    let attributes = DefaultAttributePolicy::test_attributes();
    assert_eq!(attributes, [TestAttribute::new("rstest::rstest")]);
    let rendered: Vec<_> = attributes
        .iter()
        .copied()
        .map(TestAttribute::render)
        .collect();
    assert_eq!(rendered, vec!["#[rstest::rstest]"]);
}

#[test]
fn attribute_rendering_preserves_order() {
    let attributes = [
        TestAttribute::new("rstest::rstest"),
        TestAttribute::with_arguments("tokio::test", "flavor = \"current_thread\""),
    ];
    let rendered: Vec<_> = attributes.into_iter().map(TestAttribute::render).collect();
    assert_eq!(
        rendered,
        vec![
            "#[rstest::rstest]",
            "#[tokio::test(flavor = \"current_thread\")]",
        ]
    );
}
