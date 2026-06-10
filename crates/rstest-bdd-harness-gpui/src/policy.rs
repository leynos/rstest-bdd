//! GPUI attribute policy for generated scenario tests.

use rstest_bdd_harness::{AttributePolicy, TestAttribute};

/// Attribute policy emitting GPUI test attributes.
///
/// This policy emits `#[rstest::rstest]` followed by `#[gpui::test]`.
///
/// # Examples
///
/// ```
/// use rstest_bdd_harness::AttributePolicy;
/// use rstest_bdd_harness_gpui::GpuiAttributePolicy;
///
/// let attrs = GpuiAttributePolicy::test_attributes();
/// assert_eq!(attrs.len(), 2);
/// assert_eq!(attrs[0].render(), "#[rstest::rstest]");
/// assert_eq!(attrs[1].render(), "#[gpui::test]");
/// ```
pub struct GpuiAttributePolicy;

const GPUI_TEST_ATTRIBUTES: [TestAttribute; 2] = [
    TestAttribute::new("rstest::rstest"),
    TestAttribute::new("gpui::test"),
];

impl AttributePolicy for GpuiAttributePolicy {
    fn test_attributes() -> &'static [TestAttribute] {
        &GPUI_TEST_ATTRIBUTES
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the GPUI attribute policy.
    //!
    //! The emit/render/"rstest is first" invariants are asserted by the
    //! shared conformance check in `rstest-bdd-harness`; only the expected
    //! rendered attributes are crate-specific.

    use super::GpuiAttributePolicy;
    use rstest_bdd_harness::policy_conformance::assert_attribute_policy_conformance;

    #[test]
    fn gpui_policy_conforms_to_attribute_policy_contract() {
        assert_attribute_policy_conformance::<GpuiAttributePolicy>(&[
            "#[rstest::rstest]",
            "#[gpui::test]",
        ]);
    }
}
