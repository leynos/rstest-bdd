//! Runtime integration tests for ADR-008 harness-led attribute-policy
//! defaults and their error paths in the GPUI harness crate.
//!
//! Unlike the snapshot and `RSTEST_BDD_RUN_MACROTEST`-gated expansion tests,
//! these run unconditionally under `cargo test` / `nextest` and assert
//! observable runtime behaviour:
//!
//! - A harness whose `HarnessAdapter::run` returns `Err` propagates the
//!   `harness failed to initialize scenario: ...` panic emitted by the
//!   expanded macro, and the scenario body never runs. This path needs no
//!   native GPUI runtime, so it is not feature-gated.
//! - `harness = GpuiHarness` without `attributes = ...` runs through the
//!   inferred `GpuiAttributePolicy` path with a live `TestAppContext`. This
//!   requires the native GPUI test runtime, so it shares the
//!   `native-gpui-tests` gate (and `#[serial]` discipline) with the rest of
//!   the GPUI scenario suite.

use rstest_bdd_macros::{given, scenario};

include!("../../rstest-bdd-harness/tests/support/failing_harness_error_path.rs");

// --- Failing-harness error path (no native GPUI runtime required) --------

#[given("a step that must never run")]
fn step_that_must_never_run() {
    unreachable!("the failing harness must abort the scenario before steps run");
}

failing_harness_error_path_scenario!();

// --- Inferred-policy happy path (requires the native GPUI runtime) -------

#[cfg(feature = "native-gpui-tests")]
mod native {
    //! Inferred-policy coverage that drives the real GPUI test runtime.

    use rstest_bdd_macros::{given, scenario, then, when};
    use serial_test::serial;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    static CONTEXT_POINTER: AtomicUsize = AtomicUsize::new(0);
    static CONTEXT_MUTATED: AtomicBool = AtomicBool::new(false);

    #[given("the inferred GPUI context is observed")]
    async fn inferred_gpui_context_is_observed(
        #[from(rstest_bdd_harness_context)] context: &gpui::TestAppContext,
    ) {
        // Receiving the reserved harness-context fixture proves the
        // inferred policy + harness pairing injected the GPUI context.
        CONTEXT_POINTER.store(std::ptr::from_ref(context) as usize, Ordering::SeqCst);
        CONTEXT_MUTATED.store(false, Ordering::SeqCst);
        assert!(context.test_function_name().is_none());
        assert!(
            !context.did_prompt_for_new_path(),
            "freshly-injected GPUI context must not have prompted for a new path"
        );
        std::future::ready(()).await;
    }

    #[when("the inferred GPUI context is mutated")]
    async fn inferred_gpui_context_is_mutated(
        #[from(rstest_bdd_harness_context)] context: &mut gpui::TestAppContext,
    ) {
        assert_eq!(
            std::ptr::from_ref(context) as usize,
            CONTEXT_POINTER.load(Ordering::SeqCst),
            "harness should inject one stable TestAppContext instance"
        );
        context.on_quit(|| {});
        CONTEXT_MUTATED.store(true, Ordering::SeqCst);
        std::future::ready(()).await;
    }

    #[then("the inferred GPUI context remains available")]
    async fn inferred_gpui_context_remains_available(
        #[from(rstest_bdd_harness_context)] context: &gpui::TestAppContext,
    ) {
        assert_eq!(
            std::ptr::from_ref(context) as usize,
            CONTEXT_POINTER.load(Ordering::SeqCst),
            "later steps should observe the same injected TestAppContext"
        );
        assert!(
            CONTEXT_MUTATED.load(Ordering::SeqCst),
            "mutations through &mut TestAppContext should be visible later"
        );
        assert!(
            !context.did_prompt_for_new_path(),
            "later steps should observe the same unprompted GPUI context"
        );
        let _executor = context.executor();
        std::future::ready(()).await;
    }

    /// `harness = GpuiHarness` with no `attributes = ...`: the macro infers
    /// `GpuiAttributePolicy` (ADR-008) and the step observes the injected
    /// `TestAppContext` at runtime. Async steps force the macro to execute
    /// the scenario body through the async step path, while the reserved
    /// fixture proves the context came from `GpuiHarness` rather than from
    /// `GpuiAttributePolicy` alone.
    #[scenario(
        path = "tests/features/harness_led_defaults.feature",
        name = "Inferred GPUI policy provides the test context",
        harness = rstest_bdd_harness_gpui::GpuiHarness,
    )]
    #[serial]
    fn inferred_policy_runs_scenario_through_gpui_harness() {}
}
