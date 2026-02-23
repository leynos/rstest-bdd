//! Tokio attribute policy for generated scenario tests.

use rstest_bdd_harness::{AttributePolicy, TestAttribute};

/// Attribute policy emitting Tokio current-thread test attributes.
///
/// This policy emits `#[rstest::rstest]` followed by
/// `#[tokio::test(flavor = "current_thread")]`, enabling async scenario
/// execution under a single-threaded Tokio runtime.
///
/// # Examples
///
/// ```
/// use rstest_bdd_harness::AttributePolicy;
/// use rstest_bdd_harness_tokio::TokioAttributePolicy;
///
/// let attrs = TokioAttributePolicy::test_attributes();
/// assert_eq!(attrs.len(), 2);
/// assert_eq!(attrs[0].render(), "#[rstest::rstest]");
/// assert_eq!(
///     attrs[1].render(),
///     "#[tokio::test(flavor = \"current_thread\")]",
/// );
/// ```
pub struct TokioAttributePolicy;

const TOKIO_TEST_ATTRIBUTES: [TestAttribute; 2] = [
    TestAttribute::new("rstest::rstest"),
    TestAttribute::with_arguments("tokio::test", "flavor = \"current_thread\""),
];

impl AttributePolicy for TokioAttributePolicy {
    fn test_attributes() -> &'static [TestAttribute] {
        &TOKIO_TEST_ATTRIBUTES
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the Tokio attribute policy.

    use super::{TOKIO_TEST_ATTRIBUTES, TokioAttributePolicy};
    use rstest_bdd_harness::{AttributePolicy, TestAttribute};

    #[test]
    fn tokio_policy_emits_rstest_and_tokio_test() {
        let attributes = TokioAttributePolicy::test_attributes();
        assert_eq!(attributes, TOKIO_TEST_ATTRIBUTES);
    }

    #[test]
    fn tokio_policy_renders_correct_attributes() {
        let attributes = TokioAttributePolicy::test_attributes();
        let rendered: Vec<_> = attributes
            .iter()
            .copied()
            .map(TestAttribute::render)
            .collect();
        assert_eq!(
            rendered,
            vec![
                "#[rstest::rstest]",
                "#[tokio::test(flavor = \"current_thread\")]"
            ]
        );
    }

    #[test]
    fn rstest_attribute_is_first() {
        let attributes = TokioAttributePolicy::test_attributes();
        assert_eq!(attributes.first().map(|a| a.path()), Some("rstest::rstest"));
    }
}
