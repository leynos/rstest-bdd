//! Expanded output for `scenario` with Tokio harness default override.

#[rstest::rstest]
fn with_tokio_harness_default_override() {
    let __rstest_bdd_harness = <rstest_bdd_harness_tokio::TokioHarness as Default>::default();
    <rstest_bdd_harness_tokio::TokioHarness as rstest_bdd_harness::HarnessAdapter>::run(
        &__rstest_bdd_harness,
        __rstest_bdd_request,
    )
    .unwrap_or_else(|err| {
        panic!("harness failed to initialize scenario: {err}")
    })
}
