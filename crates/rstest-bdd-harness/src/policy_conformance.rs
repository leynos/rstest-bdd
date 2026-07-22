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
/// 1. **Emit** — the policy emits exactly `expected_rendered.len()`
///    attributes.
/// 2. **Render** — each attribute renders to the corresponding entry of
///    `expected_rendered`, in order.
/// 3. **rstest is first** — the first attribute path is `rstest::rstest`, so
///    fixture expansion precedes the runtime-specific test macro.
///
/// # Panics
///
/// Panics with a descriptive message when any invariant is violated; this is
/// a test helper and is expected to run inside `#[test]` functions.
///
/// # Examples
///
/// ```
/// use rstest_bdd_harness::DefaultAttributePolicy;
/// use rstest_bdd_harness::policy_conformance::assert_attribute_policy_conformance;
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
        "`{RSTEST_ATTRIBUTE_PATH}` must be the first attribute so fixture \
         expansion precedes the runtime test macro",
    );
}
