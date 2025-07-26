use rstest_bdd_macros::{given, scenario, then, when};
use serial_test::serial;
use std::sync::{LazyLock, Mutex};

static EVENTS: LazyLock<Mutex<Vec<&'static str>>> = LazyLock::new(|| Mutex::new(Vec::new()));

fn clear_events() {
    if let Ok(mut g) = EVENTS.lock() {
        g.clear();
    }
}

#[given("a precondition")]
fn precondition() {
    clear_events();
    let mut guard = match EVENTS.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    guard.push("precondition");
}

#[when("an action occurs")]
fn action() {
    let mut guard = match EVENTS.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    guard.push("action");
}

#[then("a result is produced")]
fn result() {
    let mut guard = match EVENTS.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    guard.push("result");
}

#[scenario("tests/features/web_search.feature")]
#[serial]
fn simple_search() {
    let events = match EVENTS.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    assert_eq!(events.as_slice(), ["precondition", "action", "result"]);
    drop(events);
    clear_events();
}

#[scenario(path = "tests/features/multi.feature", index = 1)]
#[serial]
fn second_scenario() {
    let events = match EVENTS.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    assert_eq!(events.as_slice(), ["precondition", "action", "result"]);
    drop(events);
    clear_events();
}

#[scenario(path = "tests/features/web_search.feature", index = 0)]
#[serial]
fn explicit_syntax() {
    let events = match EVENTS.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    assert_eq!(events.as_slice(), ["precondition", "action", "result"]);
    drop(events);
    clear_events();
}

#[scenario(path = "tests/features/unmatched.feature")]
#[should_panic(expected = "Step not found")]
#[serial]
fn unmatched_feature() {
    clear_events();
}
