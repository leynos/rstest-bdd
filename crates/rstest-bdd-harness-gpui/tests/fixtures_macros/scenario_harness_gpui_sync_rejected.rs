//! Compile-fail fixture: `GpuiHarness` combined with `async fn` is rejected.
//!
//! GPUI scenarios must be declared as synchronous functions: the harness injects
//! `#[gpui::test]` and runs the scenario on the GPUI test thread. Writing
//! `async fn` on the scenario body is a mistake users might make when
//! copy-pasting Tokio-style examples.
//!
//! Note: the `scenario` macro currently emits the same diagnostic wording as
//! the Tokio harness rejection path (mentioning `TokioHarness`); see
//! `rstest-bdd-macros` if that message is refined for GPUI callers.
use rstest_bdd_macros::{given, scenario, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(
    path = "basic.feature",
    harness = rstest_bdd_harness_gpui::GpuiHarness,
)]
async fn async_with_gpui_harness() {}

const _: &str = include_str!("basic.feature");

fn main() {}
