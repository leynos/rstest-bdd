//! Shared conformance check for [`AttributePolicy`] implementations.
//!
//! Harness adapter crates implement small, deliberately independent
//! attribute policies whose test scaffolding would otherwise be duplicated
//! line-for-line. This module provides the canonical conformance check: each
//! policy crate supplies only its expected rendered attributes and gets the
//! emit / render / "rstest is first" invariants for free.

use crate::policy::{AttributePolicy, TestAttribute};

/// Attribute path that must lead every policy's attribute list so `rstest`
/// expands fixtures before the runtime-specific test macro.
const RSTEST_ATTRIBUTE_PATH: &str = "rstest::rstest";

/// Assert that policy `P` conforms to the attribute-policy contract.
///
/// The check pins three invariants shared by every first-party policy:
///
/// 1. **Emit** — the policy emits exactly `expected_rendered.len()` attributes.
/// 2. **Render** — each attribute renders to the corresponding entry of `expected_rendered`, in
///    order.
/// 3. **rstest is first** — the first attribute path is `rstest::rstest`, so fixture expansion
///    precedes the runtime-specific test macro.
///
/// # Panics
///
/// Panics with a descriptive message when any invariant is violated; this is
/// a test helper and is expected to run inside `#[test]` functions.
///
/// # Examples
///
/// ```
/// use rstest_bdd_harness::{
///     DefaultAttributePolicy,
///     policy_conformance::assert_attribute_policy_conformance,
/// };
///
/// assert_attribute_policy_conformance::<DefaultAttributePolicy>(&["#[rstest::rstest]"]);
/// ```
pub fn assert_attribute_policy_conformance<P: AttributePolicy>(expected_rendered: &[&str]) {
    let attributes = P::test_attributes();
    assert_eq!(
        attributes.len(),
        expected_rendered.len(),
        "policy must emit exactly {} attribute(s), got {}",
        expected_rendered.len(),
        attributes.len(),
    );

    let rendered: Vec<String> = attributes
        .iter()
        .copied()
        .map(TestAttribute::render)
        .collect();
    assert_eq!(
        rendered, expected_rendered,
        "policy attributes must render to the expected list, in order",
    );

    assert_eq!(
        attributes.first().map(|attribute| attribute.path()),
        Some(RSTEST_ATTRIBUTE_PATH),
        concat!(
            "`{}` must be the first attribute so fixture ",
            "expansion precedes the runtime test macro",
        ),
        RSTEST_ATTRIBUTE_PATH,
    );
}

#[cfg(test)]
mod tests {
    //! The conformance helper is itself conformance-checked here: a
    //! deliberately broken policy must make it panic, so a no-op or weakened
    //! helper cannot pass on the happy path alone.

    use super::assert_attribute_policy_conformance;
    use crate::policy::{AttributePolicy, TestAttribute};

    const EXPECTED: &[&str] = &["#[rstest::rstest]", "#[gpui::test]"];
    const GOOD: [TestAttribute; 2] = [
        TestAttribute::new("rstest::rstest"),
        TestAttribute::new("gpui::test"),
    ];
    const WRONG_COUNT: [TestAttribute; 1] = [TestAttribute::new("rstest::rstest")];
    const WRONG_RENDER: [TestAttribute; 2] = [
        TestAttribute::new("rstest::rstest"),
        TestAttribute::new("tokio::test"),
    ];
    const RSTEST_NOT_FIRST: [TestAttribute; 2] = [
        TestAttribute::new("gpui::test"),
        TestAttribute::new("rstest::rstest"),
    ];

    struct GoodPolicy;
    impl AttributePolicy for GoodPolicy {
        fn test_attributes() -> &'static [TestAttribute] { &GOOD }
    }

    struct WrongCountPolicy;
    impl AttributePolicy for WrongCountPolicy {
        fn test_attributes() -> &'static [TestAttribute] { &WRONG_COUNT }
    }

    struct WrongRenderPolicy;
    impl AttributePolicy for WrongRenderPolicy {
        fn test_attributes() -> &'static [TestAttribute] { &WRONG_RENDER }
    }

    struct RstestNotFirstPolicy;
    impl AttributePolicy for RstestNotFirstPolicy {
        fn test_attributes() -> &'static [TestAttribute] { &RSTEST_NOT_FIRST }
    }

    #[test]
    fn accepts_a_conforming_policy() {
        // Guards against an over-strict helper that rejects valid policies.
        assert_attribute_policy_conformance::<GoodPolicy>(EXPECTED);
    }

    #[test]
    #[should_panic(expected = "must emit exactly")]
    fn rejects_wrong_attribute_count() {
        assert_attribute_policy_conformance::<WrongCountPolicy>(EXPECTED);
    }

    #[test]
    #[should_panic(expected = "render to the expected list")]
    fn rejects_wrong_rendered_attribute() {
        assert_attribute_policy_conformance::<WrongRenderPolicy>(EXPECTED);
    }

    #[test]
    #[should_panic(expected = "must be the first attribute")]
    fn rejects_rstest_not_first() {
        // Count and render match; only the ordering (rstest first) is wrong.
        assert_attribute_policy_conformance::<RstestNotFirstPolicy>(&[
            "#[gpui::test]",
            "#[rstest::rstest]",
        ]);
    }
}
