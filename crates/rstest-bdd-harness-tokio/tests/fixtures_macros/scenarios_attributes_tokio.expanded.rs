//! Expanded output for `scenarios!` with Tokio attribute defaults.

#[tokio::test(flavor = "current_thread")]
async fn tokio_scenarios_macro_uses_harness_led_defaults() {
    precondition();
    action();
    async_result().await;
}
