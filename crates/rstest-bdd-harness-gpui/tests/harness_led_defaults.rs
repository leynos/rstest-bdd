//! Runtime integration tests for ADR-008 harness-led attribute-policy
//! defaults and their error paths in the GPUI harness crate.
//!
//! Unlike the snapshot and `RSTEST_BDD_RUN_MACROTEST`-gated expansion tests,
//! these run unconditionally under `cargo test` / `nextest` and assert
//! observable runtime behaviour:
//!
//! - A harness whose `HarnessAdapter::run` returns `Err` propagates the `harness failed to
//!   initialize scenario: ...` panic emitted by the expanded macro, and the scenario body never
//!   runs. This path needs no native GPUI runtime, so it is not feature-gated.
//! - `harness = GpuiHarness` without `attributes = ...` runs through the inferred
//!   `GpuiAttributePolicy` path with a live `TestAppContext`. This requires the native GPUI test
//!   runtime, so it shares the `native-gpui-tests` gate (and `#[serial]` discipline) with the rest
//!   of the GPUI scenario suite. Cross-step context identity is proved through a single recorded
//!   address whose lifetime is owned by a fixture guard, so success, failure, and skip paths all
//!   clear it before the next serial scenario runs — the same reset protocol as
//!   `tests/stateful_window.rs`.

use rstest_bdd_macros::{given, scenario};

#[macro_use]
#[path = "../../rstest-bdd-harness/tests/support/failing_harness_error_path.rs"]
mod failing_harness_error_path;

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

    use std::sync::atomic::{AtomicUsize, Ordering};

    use rstest::fixture;
    use rstest_bdd_macros::{given, scenario, then, when};
    use serial_test::serial;

    /// Address of the `TestAppContext` the harness injected into the current
    /// scenario, recorded solely so later steps can assert pointer identity.
    ///
    /// Ownership protocol: the `Given` step is the sole writer and stores the
    /// address before any reader runs; the `When` and `Then` steps are readers
    /// only. The value is never dereferenced, so it cannot outlive the context
    /// it describes in any meaningful sense. [`ContextPointerCleanup`] clears
    /// it before assignment and again at scenario teardown, so a failed,
    /// panicking, or skipped scenario cannot leak a stale address into the
    /// next scenario on the reused `#[serial]` test thread. The sentinel value
    /// `0` never matches a live reference, so a missing `Given` step fails the
    /// identity assertions loudly rather than passing on stale data.
    static CONTEXT_POINTER: AtomicUsize = AtomicUsize::new(0);

    const UNSET_CONTEXT_POINTER: usize = 0;

    fn reset_context_pointer() { CONTEXT_POINTER.store(UNSET_CONTEXT_POINTER, Ordering::SeqCst); }

    /// Fixture guard that resets [`CONTEXT_POINTER`] around each scenario.
    #[derive(Clone, Debug)]
    struct ContextPointerCleanup;

    impl Drop for ContextPointerCleanup {
        fn drop(&mut self) { reset_context_pointer(); }
    }

    #[rstest_bdd_test_macros::allow_fixture_expansion_lints]
    #[fixture]
    fn context_pointer_cleanup() -> ContextPointerCleanup {
        // Reset before the scenario assigns its own address so a reused serial
        // thread cannot observe an address left by an earlier scenario.
        reset_context_pointer();
        ContextPointerCleanup
    }

    #[given("the inferred GPUI context is observed")]
    async fn inferred_gpui_context_is_observed(
        #[from(rstest_bdd_harness_context)] context: &gpui::TestAppContext,
    ) {
        // Receiving the reserved harness-context fixture proves the
        // inferred policy + harness pairing injected the GPUI context.
        assert_eq!(
            CONTEXT_POINTER.swap(std::ptr::from_ref(context) as usize, Ordering::SeqCst),
            UNSET_CONTEXT_POINTER,
            "cleanup fixture should have cleared the recorded context address"
        );
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
        context.add_window_view(|_| ());
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
        assert_eq!(
            context.windows().len(),
            1,
            "later steps should observe the window added through the GPUI context"
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
    ///
    /// `#[serial]` keeps the scenario off concurrent GPUI runtimes, and
    /// `context_pointer_cleanup` owns the lifetime of the recorded context
    /// address so the shared static cannot leak across scenarios.
    #[scenario(
        path = "tests/features/harness_led_defaults.feature",
        name = "Inferred GPUI policy provides the test context",
        harness = rstest_bdd_harness_gpui::GpuiHarness,
    )]
    #[serial]
    fn inferred_policy_runs_scenario_through_gpui_harness(
        #[from(context_pointer_cleanup)] _cleanup: ContextPointerCleanup,
    ) {
    }
}
