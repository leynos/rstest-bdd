//! Expanded output for `scenario` with GPUI harness default override.

#[rstest::rstest]
fn with_gpui_harness_default_override() {
    let __rstest_bdd_harness = <rstest_bdd_harness_gpui::GpuiHarness as Default>::default();
    <rstest_bdd_harness_gpui::GpuiHarness as rstest_bdd_harness::HarnessAdapter>::run(
        &__rstest_bdd_harness,
        __rstest_bdd_request,
    )
    .unwrap_or_else(|err| {
        panic!("harness failed to initialise scenario: {err}")
    })
}
