//! Default synchronous harness implementation.

use crate::adapter::HarnessAdapter;
use crate::runner::ScenarioRunRequest;

/// Panic message used by `StdHarness` panic-propagation tests.
pub const STD_HARNESS_PANIC_MESSAGE: &str = "std harness panic propagation";

/// Framework-agnostic synchronous harness.
///
/// `StdHarness` executes the scenario runner directly without an async runtime
/// or UI harness.
#[derive(Debug, Clone, Copy, Default)]
pub struct StdHarness;

impl StdHarness {
    /// Creates a new synchronous harness instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl HarnessAdapter for StdHarness {
    fn run<T>(&self, request: ScenarioRunRequest<'_, T>) -> T {
        request.run()
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the synchronous standard harness.

    use std::any::Any;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use rstest::{fixture, rstest};

    use super::StdHarness;
    use crate::{
        HarnessAdapter, STD_HARNESS_PANIC_MESSAGE, ScenarioMetadata, ScenarioRunRequest,
        ScenarioRunner,
    };

    #[fixture]
    fn harness() -> StdHarness {
        StdHarness::new()
    }

    #[fixture]
    fn metadata() -> ScenarioMetadata {
        ScenarioMetadata::new(
            "tests/features/simple.feature",
            "Runs synchronously",
            4,
            vec!["@sync".to_string()],
        )
    }

    fn panic_payload_matches(payload: &(dyn Any + Send), expected: &str) -> bool {
        payload
            .downcast_ref::<&str>()
            .is_some_and(|message| *message == expected)
            || payload
                .downcast_ref::<String>()
                .is_some_and(|message| message == expected)
    }

    #[rstest]
    fn std_harness_runs_request(harness: StdHarness, metadata: ScenarioMetadata) {
        let request = ScenarioRunRequest::new(metadata, ScenarioRunner::new(|| 21 * 2));
        assert_eq!(harness.run(request), 42);
    }

    #[rstest]
    fn std_harness_propagates_runner_panics(harness: StdHarness) {
        let request = ScenarioRunRequest::new(
            ScenarioMetadata::default(),
            ScenarioRunner::new(|| panic!("{STD_HARNESS_PANIC_MESSAGE}")),
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
