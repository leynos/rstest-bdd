//! Behavioural tests for Tokio attribute policy output.

use rstest_bdd_harness::{AttributePolicy, TestAttribute};
use rstest_bdd_harness_tokio::TokioAttributePolicy;

#[test]
fn tokio_policy_emits_rstest_and_tokio_test_attributes() {
    let attributes = TokioAttributePolicy::test_attributes();
    assert_eq!(
        attributes,
        [
            TestAttribute::new("rstest::rstest"),
            TestAttribute::with_arguments("tokio::test", "flavor = \"current_thread\"",),
        ]
    );
    let rendered: Vec<_> = attributes
        .iter()
        .copied()
        .map(TestAttribute::render)
        .collect();
    assert_eq!(
        rendered,
        vec![
            "#[rstest::rstest]",
            "#[tokio::test(flavor = \"current_thread\")]",
        ]
    );
}

#[test]
fn tokio_policy_attributes_preserve_order() {
    let attributes = TokioAttributePolicy::test_attributes();
    assert_eq!(attributes.first().map(|a| a.path()), Some("rstest::rstest"));
    assert_eq!(attributes.get(1).map(|a| a.path()), Some("tokio::test"));
}
