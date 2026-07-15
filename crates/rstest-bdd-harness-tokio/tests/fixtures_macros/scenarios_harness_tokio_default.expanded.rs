//! Expanded output for `scenarios!` with the Tokio harness-led default policy.

#[rstest::rstest]
fn tokio_scenarios_macro_uses_harness_led_defaults() {
    let __rstest_bdd_harness = <rstest_bdd_harness_tokio::TokioHarness as Default>::default();
    <rstest_bdd_harness_tokio::TokioHarness as rstest_bdd_harness::HarnessAdapter>::run(
        &__rstest_bdd_harness,
        __rstest_bdd_request,
    )
    .unwrap_or_else(|err| {
        panic!("harness failed to initialize scenario: {err}")
    })
}
