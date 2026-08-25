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

/// Attributes emitted for generated GPUI scenario tests.
const GPUI_TEST_ATTRIBUTES: [TestAttribute; 2] = [
    TestAttribute::new("rstest::rstest"),
    TestAttribute::new("gpui::test"),
];

impl AttributePolicy for GpuiAttributePolicy {
    fn test_attributes() -> &'static [TestAttribute] { &GPUI_TEST_ATTRIBUTES }
}

// The attribute-policy conformance check lives in
// `tests/attribute_policy_behaviour.rs` rather than an in-module
// `#[cfg(test)]` block: this crate's library target sets `test = false`, so an
// in-module test would never be compiled or run.
