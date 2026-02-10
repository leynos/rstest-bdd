//! Behavioural tests verifying that `#[scenario]` accepts the `harness` and
//! `attributes` parameters and generates working test functions.

use rstest_bdd_macros::{given, scenario, then, when};
use serial_test::serial;
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
    with_locked_events(|events| {
        assert_eq!(events.as_slice(), ["precondition", "action", "result"]);
    });
    clear_events();
}

#[scenario(
    path = "tests/features/web_search.feature",
    attributes = rstest_bdd_harness::DefaultAttributePolicy,
)]
#[serial]
fn scenario_with_attributes() {
    with_locked_events(|events| {
        assert_eq!(events.as_slice(), ["precondition", "action", "result"]);
    });
    clear_events();
}

#[scenario(
    path = "tests/features/web_search.feature",
    harness = rstest_bdd_harness::StdHarness,
    attributes = rstest_bdd_harness::DefaultAttributePolicy,
)]
#[serial]
fn scenario_with_harness_and_attributes() {
    with_locked_events(|events| {
        assert_eq!(events.as_slice(), ["precondition", "action", "result"]);
    });
    clear_events();
}
