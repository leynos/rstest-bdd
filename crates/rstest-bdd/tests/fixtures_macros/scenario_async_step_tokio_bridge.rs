//! Compile-pass fixture proving generated async wrappers use rstest-bdd's
//! Tokio bridge rather than requiring the consuming crate to depend on Tokio.
use rstest_bdd_macros::{given, scenario};

#[given("an asynchronous step")]
async fn asynchronous_step() {}

#[scenario(path = "async_step_tokio_bridge.feature")]
fn synchronous_scenario() {}

const _: &str = include_str!("async_step_tokio_bridge.feature");

fn main() {}
