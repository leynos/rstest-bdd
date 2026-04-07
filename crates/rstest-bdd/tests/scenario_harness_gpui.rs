//! Integration tests verifying that `#[scenario]` works with the GPUI harness
//! adapter and attribute policy from `rstest-bdd-harness-gpui`.
#![cfg(feature = "gpui-harness-tests")]

use rstest_bdd_macros::{given, scenario, scenarios, then, when};
use serial_test::serial;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static CONTEXT_POINTER: AtomicUsize = AtomicUsize::new(0);
static CONTEXT_MUTATED: AtomicBool = AtomicBool::new(false);
static GPUI_POLICY_RAN: AtomicBool = AtomicBool::new(false);
static GPUI_SCENARIOS_MACRO_RAN: AtomicBool = AtomicBool::new(false);

#[expect(
    clippy::unnecessary_wraps,
    reason = "exercise gpui::test termination handling for Result-returning tests"
)]
#[gpui::test]
fn gpui_test_preserves_declared_name(context: &gpui::TestAppContext) -> Result<(), &'static str> {
    assert_eq!(
        context.test_function_name(),
        Some("gpui_test_preserves_declared_name")
    );
    Ok(())
}

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

#[given("a GPUI scenarios macro policy run starts")]
fn gpui_scenarios_macro_policy_run_starts() {
    GPUI_SCENARIOS_MACRO_RAN.store(true, Ordering::SeqCst);
}

#[then("the GPUI scenarios macro policy run completed")]
fn gpui_scenarios_macro_policy_run_completed() {
    // NOTE: This is a minimal runtime smoke test verifying that scenarios! with
    // GPUI attribute policy can execute steps. The attribute-policy application itself
    // is verified by the corresponding trybuild compile-pass fixture at
    // crates/rstest-bdd/tests/fixtures_macros/scenarios_attributes_gpui.rs, which
    // ensures the generated code compiles with #[gpui::test] attributes.
    // A stronger runtime assertion would require accessing GPUI-specific state
    // (e.g., TestAppContext), which is only available when using GpuiHarness,
    // not when using GpuiAttributePolicy alone.
    assert!(
        GPUI_SCENARIOS_MACRO_RAN.load(Ordering::SeqCst),
        "scenarios! should execute under the GPUI attribute policy"
    );
    GPUI_SCENARIOS_MACRO_RAN.store(false, Ordering::SeqCst);
}

#[given("a GPUI test context can be accessed")]
fn gpui_test_context_can_be_accessed(
    #[from(rstest_bdd_harness_context)] context: &gpui::TestAppContext,
) {
    // This step verifies GPUI-specific behaviour: the harness must inject a TestAppContext,
    // and the attribute policy must emit #[gpui::test] to provide the GPUI test infrastructure.
    // If either were missing, this parameter injection would fail.
    assert!(
        context.dispatcher().seed() == 0,
        "GPUI TestAppContext should be injected with default seed"
    );
}

#[then("the GPUI test context is valid")]
fn gpui_test_context_is_valid(#[from(rstest_bdd_harness_context)] context: &gpui::TestAppContext) {
    // Verify we can access GPUI-specific functionality. This assertion would fail
    // if GpuiAttributePolicy were not correctly applied, because without #[gpui::test]
    // the GPUI test infrastructure wouldn't be available to create the TestAppContext.
    assert_eq!(
        context.dispatcher().seed(),
        0,
        "GPUI TestAppContext dispatcher should have expected seed value"
    );
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

#[scenario(
    path = "tests/features/gpui_harness.feature",
    name = "GPUI attribute policy runs without harness",
    attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy,
)]
#[gpui::test]
#[serial]
fn scenario_gpui_attribute_policy_dedup() {}

scenarios!(
    "tests/features/gpui_policy_scenarios",
    attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy,
);

scenarios!(
    "tests/features/gpui_harness_policy_scenarios",
    harness = rstest_bdd_harness_gpui::GpuiHarness,
    attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy,
);
