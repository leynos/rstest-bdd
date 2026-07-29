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
    //!
    //! The emit/render/"rstest is first" invariants are asserted by the
    //! shared conformance check in `rstest-bdd-harness`; only the expected
    //! rendered attributes are crate-specific.

    use super::TokioAttributePolicy;
    use rstest_bdd_harness::policy_conformance::assert_attribute_policy_conformance;

    #[test]
    fn tokio_policy_conforms_to_attribute_policy_contract() {
        assert_attribute_policy_conformance::<TokioAttributePolicy>(&[
            "#[rstest::rstest]",
            "#[tokio::test(flavor = \"current_thread\")]",
        ]);
    }
}
