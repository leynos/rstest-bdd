//! Expanded output for `scenarios!` with the GPUI harness-led default policy.

#[rstest::rstest]
fn basic_discovers_a_gpui_scenario() {
    let __rstest_bdd_harness = <rstest_bdd_harness_gpui::GpuiHarness as Default>::default();
    <rstest_bdd_harness_gpui::GpuiHarness as rstest_bdd_harness::HarnessAdapter>::run(
        &__rstest_bdd_harness,
        __rstest_bdd_request,
    )
    .unwrap_or_else(|err| {
        panic!("harness failed to initialize scenario: {err}")
    })
}
