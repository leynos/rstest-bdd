//! Behavioural tests verifying that `#[scenario]` accepts the `harness` and
//! `attributes` parameters and delegates execution through the harness adapter.

use rstest_bdd_harness::{HarnessAdapter, ScenarioRunRequest};
use rstest_bdd_macros::{given, scenario, then, when};
use serial_test::serial;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard};

static EVENTS: LazyLock<Mutex<Vec<&'static str>>> = LazyLock::new(|| Mutex::new(Vec::new()));

fn get_events_guard() -> MutexGuard<'static, Vec<&'static str>> {
    match EVENTS.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    }
}

fn clear_events() {
    let mut g = get_events_guard();
    g.clear();
}

fn with_locked_events<F, R>(f: F) -> R
where
    F: FnOnce(&mut Vec<&'static str>) -> R,
{
    let mut guard = get_events_guard();
    f(&mut guard)
}

/// Assert that the expected steps ran in order, then clear for the next test.
fn assert_and_clear_events() {
    with_locked_events(|events| {
        assert_eq!(events.as_slice(), ["precondition", "action", "result"]);
    });
    clear_events();
}

#[given("a precondition")]
fn precondition() {
    clear_events();
    with_locked_events(|events| events.push("precondition"));
}

#[when("an action occurs")]
fn action() {
    with_locked_events(|events| events.push("action"));
}

#[then("a result is produced")]
fn result() {
    with_locked_events(|events| events.push("result"));
}

#[scenario(
    path = "tests/features/web_search.feature",
    harness = rstest_bdd_harness::StdHarness,
)]
#[serial]
fn scenario_with_harness() {
    assert_and_clear_events();
}

#[scenario(
    path = "tests/features/web_search.feature",
    attributes = rstest_bdd_harness::DefaultAttributePolicy,
)]
#[serial]
fn scenario_with_attributes() {
    assert_and_clear_events();
}

#[scenario(
    path = "tests/features/web_search.feature",
    harness = rstest_bdd_harness::StdHarness,
    attributes = rstest_bdd_harness::DefaultAttributePolicy,
)]
#[serial]
fn scenario_with_harness_and_attributes() {
    assert_and_clear_events();
}

// ---------------------------------------------------------------------------
// Custom harness tests verifying that execution is actually delegated
// ---------------------------------------------------------------------------

static HARNESS_INVOKED: AtomicBool = AtomicBool::new(false);

/// A harness that records whether `run()` was called and validates metadata.
#[derive(Default)]
struct RecordingHarness;

impl HarnessAdapter for RecordingHarness {
    fn run<T>(&self, request: ScenarioRunRequest<'_, T>) -> T {
        HARNESS_INVOKED.store(true, Ordering::SeqCst);
        let meta = request.metadata();
        assert!(
            !meta.feature_path().is_empty(),
            "harness should receive non-empty feature path"
        );
        assert!(
            !meta.scenario_name().is_empty(),
            "harness should receive non-empty scenario name"
        );
        request.run()
    }
}

#[scenario(
    path = "tests/features/web_search.feature",
    harness = RecordingHarness,
)]
#[serial]
fn scenario_delegates_to_custom_harness() {
    assert!(
        HARNESS_INVOKED.load(Ordering::SeqCst),
        "RecordingHarness.run() should have been called"
    );
    HARNESS_INVOKED.store(false, Ordering::SeqCst);
    assert_and_clear_events();
}

static CAPTURED_FEATURE: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));
static CAPTURED_SCENARIO: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));

/// A harness that captures scenario metadata for later assertion.
#[derive(Default)]
struct MetadataCapturingHarness;

impl HarnessAdapter for MetadataCapturingHarness {
    fn run<T>(&self, request: ScenarioRunRequest<'_, T>) -> T {
        let meta = request.metadata();
        *CAPTURED_FEATURE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = meta.feature_path().to_string();
        *CAPTURED_SCENARIO
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = meta.scenario_name().to_string();
        request.run()
    }
}

#[scenario(
    path = "tests/features/web_search.feature",
    harness = MetadataCapturingHarness,
)]
#[serial]
fn scenario_passes_correct_metadata_to_harness() {
    let feature = CAPTURED_FEATURE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(
        feature.contains("web_search.feature"),
        "expected feature path to contain 'web_search.feature', got: {feature}"
    );
    drop(feature);

    let scenario = CAPTURED_SCENARIO
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(
        scenario.as_str(),
        "Simple search",
        "expected scenario name 'Simple search'"
    );
    drop(scenario);

    assert_and_clear_events();
}
