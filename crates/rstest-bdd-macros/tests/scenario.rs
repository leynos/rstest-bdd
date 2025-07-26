use rstest_bdd_macros::{given, scenario, then, when};
use std::sync::{LazyLock, Mutex};

static EVENTS: LazyLock<Mutex<Vec<&'static str>>> = LazyLock::new(|| Mutex::new(Vec::new()));

#[given("a precondition")]
fn precondition() {
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

#[scenario(path = "tests/features/web_search.feature")]
fn simple_search() {
    let events = match EVENTS.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    assert_eq!(events.as_slice(), ["precondition", "action", "result"]);
    drop(events);
    if let Ok(mut g) = EVENTS.lock() {
        g.clear();
    }
}

#[scenario(path = "tests/features/multi.feature", index = 1)]
fn second_scenario() {
    let events = match EVENTS.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    assert_eq!(events.as_slice(), ["precondition", "action", "result"]);
    drop(events);
    if let Ok(mut g) = EVENTS.lock() {
        g.clear();
    }
}
