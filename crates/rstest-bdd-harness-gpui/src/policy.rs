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

    use super::{GPUI_TEST_ATTRIBUTES, GpuiAttributePolicy};
    use rstest_bdd_harness::{AttributePolicy, TestAttribute};

    #[test]
    fn gpui_policy_emits_rstest_and_gpui_test() {
        let attributes = GpuiAttributePolicy::test_attributes();
        assert_eq!(attributes, GPUI_TEST_ATTRIBUTES);
    }

    #[test]
    fn gpui_policy_renders_correct_attributes() {
        let attributes = GpuiAttributePolicy::test_attributes();
        let rendered: Vec<_> = attributes
            .iter()
            .copied()
            .map(TestAttribute::render)
            .collect();
        assert_eq!(rendered, vec!["#[rstest::rstest]", "#[gpui::test]"]);
    }

    #[test]
    fn rstest_attribute_is_first() {
        let attributes = GpuiAttributePolicy::test_attributes();
        assert_eq!(attributes.first().map(|a| a.path()), Some("rstest::rstest"));
    }
}
