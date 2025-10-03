//! Behavioural tests covering the `#[scenario]` macro

use rstest::rstest;
use rstest_bdd_macros::{given, scenario, then, when};
use serial_test::serial;
use std::sync::{LazyLock, Mutex, MutexGuard};

static EVENTS: LazyLock<Mutex<Vec<&'static str>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// Return a guard to the `EVENTS` mutex, recovering from poison.
///
/// If an earlier test panicked while holding the lock the mutex
/// becomes poisoned. This helper extracts the inner guard so later
/// tests can continue to inspect and modify the shared event list.
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

/// Access the events vector under a lock and run the provided closure.
///
/// The `f` parameter is a closure that receives a mutable reference to the
/// vector of static string events and returns any result type. The lock is
/// acquired via `get_events_guard` before the closure is called.
fn with_locked_events<F, R>(f: F) -> R
where
    F: FnOnce(&mut Vec<&'static str>) -> R,
{
    let mut guard = get_events_guard();
    f(&mut guard)
}

#[given("a background step")]
fn background_step() {
    with_locked_events(|events| events.push("background"));
}

#[given("another background step")]
fn another_background_step() {
    with_locked_events(|events| events.push("another background"));
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

#[scenario("tests/features/web_search.feature")]
#[serial]
fn simple_search() {
    with_locked_events(|events| {
        assert_eq!(events.as_slice(), ["precondition", "action", "result"]);
    });
    clear_events();
}

#[rstest]
#[case::named_args("second_scenario")]
#[case::bare_path("second_scenario_bare")]
#[scenario("tests/features/multi.feature", index = 1)]
#[serial]
fn scenario_with_index(#[case] case_name: &str) {
    with_locked_events(|events| {
        assert_eq!(
            events.as_slice(),
            ["precondition", "action", "result"],
            "Test case: {case_name}"
        );
    });
    clear_events();
}

#[scenario(path = "tests/features/multi.feature", name = "Second")]
#[serial]
fn scenario_with_name_selector() {
    with_locked_events(|events| {
        assert_eq!(events.as_slice(), ["precondition", "action", "result"]);
    });
    clear_events();
}

#[scenario(path = "tests/features/web_search.feature", index = 0)]
#[serial]
fn explicit_syntax() {
    with_locked_events(|events| {
        assert_eq!(events.as_slice(), ["precondition", "action", "result"]);
    });
    clear_events();
}

#[scenario(path = "tests/features/outline.feature")]
#[serial] // EVENTS is shared, so run tests sequentially
fn outline(num: String) {
    with_locked_events(|events| {
        assert_eq!(events.as_slice(), ["precondition", "action", "result"]);
    });
    assert!(num == "1" || num == "2");
    clear_events();
}

#[scenario("tests/features/background.feature", index = 0)]
#[serial]
fn background_first() {
    with_locked_events(|events| {
        assert_eq!(
            events.as_slice(),
            ["background", "another background", "action", "result"]
        );
    });
    clear_events();
}

#[scenario("tests/features/background.feature", index = 1)]
#[serial]
fn background_second() {
    with_locked_events(|events| {
        assert_eq!(
            events.as_slice(),
            ["background", "another background", "action", "result"]
        );
    });
    clear_events();
}

#[test]
#[serial]
fn multiple_background_steps_execute_in_order() {
    clear_events();
    background_step();
    another_background_step();
    with_locked_events(|events| {
        assert_eq!(events.as_slice(), ["background", "another background"]);
    });

    clear_events();
    background_step();
    another_background_step();
    with_locked_events(|events| {
        assert_eq!(events.as_slice(), ["background", "another background"]);
    });

    clear_events();
}
