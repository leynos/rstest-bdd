//! Integration tests verifying that `#[scenario]` works with the GPUI harness
//! adapter and attribute policy from `rstest-bdd-harness-gpui`.
#![cfg(feature = "gpui-harness-tests")]

use rstest_bdd_macros::{given, scenario, then, when};
use serial_test::serial;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static CONTEXT_POINTER: AtomicUsize = AtomicUsize::new(0);
static CONTEXT_MUTATED: AtomicBool = AtomicBool::new(false);
static GPUI_POLICY_RAN: AtomicBool = AtomicBool::new(false);

#[given("a GPUI test context is injected")]
fn gpui_context_is_injected(#[from(rstest_bdd_harness_context)] context: &gpui::TestAppContext) {
    CONTEXT_POINTER.store(std::ptr::from_ref(context) as usize, Ordering::SeqCst);
    CONTEXT_MUTATED.store(false, Ordering::SeqCst);
    assert!(context.test_function_name().is_none());
}

#[when("the GPUI test context is accessed mutably")]
fn gpui_context_is_accessed_mutably(
    #[from(rstest_bdd_harness_context)] context: &mut gpui::TestAppContext,
) {
    assert_eq!(
        std::ptr::from_ref(context) as usize,
        CONTEXT_POINTER.load(Ordering::SeqCst),
        "harness should inject one stable TestAppContext instance"
    );
    context.on_quit(|| {});
    CONTEXT_MUTATED.store(true, Ordering::SeqCst);
}

#[then("the same GPUI context remains available")]
fn same_gpui_context_remains_available(
    #[from(rstest_bdd_harness_context)] context: &gpui::TestAppContext,
) {
    assert_eq!(
        std::ptr::from_ref(context) as usize,
        CONTEXT_POINTER.load(Ordering::SeqCst),
        "later steps should observe the same injected TestAppContext"
    );
    assert!(
        CONTEXT_MUTATED.load(Ordering::SeqCst),
        "mutations performed through &mut TestAppContext should be visible later"
    );
}

#[given("a plain GPUI policy scenario runs")]
fn plain_gpui_policy_scenario_runs() {
    GPUI_POLICY_RAN.store(true, Ordering::SeqCst);
}

#[then("the plain GPUI policy scenario completed")]
fn plain_gpui_policy_scenario_completed() {
    assert!(
        GPUI_POLICY_RAN.load(Ordering::SeqCst),
        "steps should execute under the GPUI attribute policy"
    );
    GPUI_POLICY_RAN.store(false, Ordering::SeqCst);
}

#[scenario(
    path = "tests/features/gpui_harness.feature",
    name = "GPUI harness injects TestAppContext",
    harness = rstest_bdd_harness_gpui::GpuiHarness,
)]
#[serial]
fn scenario_gpui_harness_injects_context() {}

#[scenario(
    path = "tests/features/gpui_harness.feature",
    name = "GPUI harness with GPUI attribute policy",
    harness = rstest_bdd_harness_gpui::GpuiHarness,
    attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy,
)]
#[serial]
fn scenario_gpui_harness_with_attribute_policy() {}

#[scenario(
    path = "tests/features/gpui_harness.feature",
    name = "GPUI attribute policy runs without harness",
    attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy,
)]
#[serial]
fn scenario_gpui_attribute_policy_without_harness() {}
