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

#[scenario(path = "tests/features/multi.feature", index = 1)]
#[serial]
fn second_scenario() {
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

#[scenario(path = "tests/features/unmatched.feature")]
#[should_panic(expected = "Step not found")]
#[serial]
fn unmatched_feature() {
    clear_events();
}
