//! Shared BDD scenario assertion proving that harness initialization failures
//! abort before step execution. Tokio and GPUI integration test modules use
//! this macro to keep their runtime error-path coverage aligned.

macro_rules! failing_harness_error_path_scenario {
    () => {
        /// A harness `run` returning `Err` must surface the macro's
        /// `harness failed to initialize scenario: ...` panic, carrying the
        /// underlying error and scenario context, and must not execute any step.
        #[scenario(
            path = "tests/features/harness_led_defaults.feature",
            name = "Failing harness initialization propagates",
            harness = rstest_bdd_harness::FailingHarness,
        )]
        #[should_panic(
            expected = "harness failed to initialize scenario: failed to build runtime: synthetic harness initialization failure"
        )]
        fn failing_harness_panics_with_meaningful_message() {}
    };
}
