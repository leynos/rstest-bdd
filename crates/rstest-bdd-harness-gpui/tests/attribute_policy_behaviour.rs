//! Behavioural tests for GPUI attribute policy output.
//!
//! The GPUI library target sets `test = false`, so the policy's conformance
//! check cannot live in an in-module `#[cfg(test)]` block — it would never be
//! compiled or run. It runs here instead, in an integration target that is
//! built under `--all-features` (which enables `native-gpui-tests`).
#![cfg(feature = "native-gpui-tests")]

use rstest_bdd_harness::policy_conformance::assert_attribute_policy_conformance;
use rstest_bdd_harness_gpui::GpuiAttributePolicy;

#[test]
fn gpui_policy_conforms_to_attribute_policy_contract() {
    assert_attribute_policy_conformance::<GpuiAttributePolicy>(&[
        "#[rstest::rstest]",
        "#[gpui::test]",
    ]);
}
