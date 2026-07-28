//! Compile-pass fixture ensuring a custom adapter may share Tokio's type name.

use rstest_bdd_harness::{HarnessAdapter, HarnessResult, ScenarioRunRequest};
use rstest_bdd_macros::{given, scenario, then, when};

mod custom {
    use super::{HarnessAdapter, HarnessResult, ScenarioRunRequest};

    #[derive(Default)]
    pub struct TokioHarness;

    impl HarnessAdapter for TokioHarness {
        type Context = ();

        fn run<T>(&self, request: ScenarioRunRequest<'_, Self::Context, T>) -> HarnessResult<T> {
            Ok(request.run_without_context())
        }
    }
}

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(
    path = "basic.feature",
    harness = custom::TokioHarness,
)]
fn with_custom_tokio_harness_name() {}

const _: &str = include_str!("basic.feature");

fn main() {}
