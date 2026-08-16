//! Default synchronous harness implementation.

use crate::{HarnessAdapter, HarnessResult, runner::StdScenarioRunRequest};

/// Framework-agnostic synchronous harness.
///
/// `StdHarness` executes the scenario runner directly without an async runtime
/// or UI harness.
#[derive(Debug, Clone, Copy, Default)]
pub struct StdHarness;

impl StdHarness {
    /// Creates a new synchronous harness instance.
    #[must_use]
    pub const fn new() -> Self { Self }
}

impl HarnessAdapter for StdHarness {
    type Context = ();

    fn run<T>(&self, request: StdScenarioRunRequest<'_, T>) -> HarnessResult<T> {
        Ok(request.run_without_context())
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the synchronous standard harness.

    use std::panic::{AssertUnwindSafe, catch_unwind};

    use rstest::{fixture, rstest};

    use super::StdHarness;
    use crate::{
        HarnessAdapter,
        ScenarioMetadata,
        ScenarioRunner,
        StdScenarioRunRequest,
        StdScenarioRunner,
        test_utils::{STD_HARNESS_PANIC_MESSAGE, panic_payload_matches},
    };

    #[rstest_bdd_test_macros::allow_fixture_expansion_lints]
    #[fixture]
    fn harness() -> StdHarness { StdHarness::new() }

    #[rstest_bdd_test_macros::allow_fixture_expansion_lints]
    #[fixture]
    fn metadata() -> ScenarioMetadata {
        ScenarioMetadata::new(
            "tests/features/simple.feature",
            "Runs synchronously",
            4,
            vec!["@sync".to_owned()],
        )
    }

    #[rstest]
    fn std_harness_runs_request(harness: StdHarness, metadata: ScenarioMetadata) {
        let request =
            StdScenarioRunRequest::new(metadata, StdScenarioRunner::new_without_context(|| 21 * 2));
        let result = match harness.run(request) {
            Ok(result) => result,
            Err(err) => panic!("std harness should not fail: {err}"),
        };
        assert_eq!(result, 42);
    }

    #[rstest]
    fn std_harness_propagates_runner_panics(harness: StdHarness, metadata: ScenarioMetadata) {
        let request = StdScenarioRunRequest::new(
            metadata,
            ScenarioRunner::new_without_context(|| panic!("{STD_HARNESS_PANIC_MESSAGE}")),
        );
        let panic_result = catch_unwind(AssertUnwindSafe(|| harness.run(request)));

        match panic_result {
            Ok(_) => panic!("expected StdHarness to propagate runner panic"),
            Err(payload) => {
                assert!(panic_payload_matches(&*payload, STD_HARNESS_PANIC_MESSAGE));
            }
        }
    }
}
