//! Integration tests verifying that `#[scenario]` works with the GPUI harness
//! adapter and attribute policy from `rstest-bdd-harness-gpui`.
#![cfg(feature = "gpui-harness-tests")]

use rstest_bdd_macros::{given, scenario, scenarios, then, when};
use serial_test::serial;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static CONTEXT_POINTER: AtomicUsize = AtomicUsize::new(0);
static CONTEXT_MUTATED: AtomicBool = AtomicBool::new(false);
static GPUI_POLICY_RAN: AtomicBool = AtomicBool::new(false);
static GPUI_SCENARIOS_MACRO_RUN_COUNT: AtomicUsize = AtomicUsize::new(0);

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
    GPUI_SCENARIOS_MACRO_RUN_COUNT.fetch_add(1, Ordering::SeqCst);
}

#[then("the GPUI scenarios macro policy run completes")]
fn gpui_scenarios_macro_policy_run_completes() {
    // NOTE: This is a minimal runtime smoke test verifying that scenarios! with
    // GPUI attribute policy can execute steps. The attribute-policy application itself
    // is verified by the corresponding trybuild compile-pass fixture at
    // crates/rstest-bdd/tests/fixtures_macros/scenarios_attributes_gpui.rs, which
    // ensures the generated code compiles with #[gpui::test] attributes.
    // A stronger runtime assertion would require accessing GPUI-specific state
    // (e.g., TestAppContext), which is only available when using GpuiHarness,
    // not when using GpuiAttributePolicy alone.
    assert!(
        GPUI_SCENARIOS_MACRO_RUN_COUNT.load(Ordering::SeqCst) > 0,
        "scenarios! should execute under the GPUI attribute policy"
    );
}

/// Asserts that a GPUI `TestAppContext` has the expected default seed value.
fn assert_gpui_context_seed_default(context: &gpui::TestAppContext) {
    assert_eq!(
        context.dispatcher().seed(),
        0,
        "GPUI TestAppContext should have default seed value"
    );
}

#[given("a GPUI test is running")]
fn gpui_test_is_running() {
    // This step simply establishes the precondition that we're in a GPUI test context.
    // The actual verification happens in the When and Then steps.
}

#[when("I access the GPUI test context")]
fn i_access_the_gpui_test_context(
    #[from(rstest_bdd_harness_context)] context: &gpui::TestAppContext,
) {
    // This step performs the action of accessing the GPUI test context.
    // The fact that this context parameter can be injected via #[from(rstest_bdd_harness_context)]
    // proves that GpuiHarness is active. We verify GPUI-specific properties to ensure
    // the infrastructure is working correctly.

    // Verify GPUI-specific invariant: default seed value
    assert_gpui_context_seed_default(context);

    // Access another GPUI-specific API: executor() returns BackgroundExecutor
    let _executor = context.executor();
}

#[then("the GPUI test context is valid")]
fn gpui_test_context_is_valid(#[from(rstest_bdd_harness_context)] context: &gpui::TestAppContext) {
    // Verify GPUI-specific behaviour by accessing APIs unique to gpui::TestAppContext.
    // These assertions would fail at compile time if the context weren't a GPUI TestAppContext,
    // proving that both the harness (which injects the context) and the attribute policy
    // (which emits #[gpui::test] to provide GPUI infrastructure) are correctly applied.

    // Verify the context has GPUI-specific properties
    assert_gpui_context_seed_default(context);

    // Verify we can access GPUI-specific methods
    assert!(
        !context.did_prompt_for_new_path(),
        "GPUI-specific did_prompt_for_new_path() method should be accessible"
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
