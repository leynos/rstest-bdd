#![cfg(unix)]

use rstest_bdd_macros::{given, scenarios, then, when};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

static EXECUTIONS: LazyLock<AtomicUsize> = LazyLock::new(|| AtomicUsize::new(0));

#[given("a symlinked directory exists")]
fn given_symlinked_directory_exists() {
    EXECUTIONS.fetch_add(1, Ordering::SeqCst);
}

#[when("the scenarios macro walks the tree")]
fn when_macro_walks_tree() {
    EXECUTIONS.fetch_add(1, Ordering::SeqCst);
}

#[then("it discovers the symlinked feature")]
fn then_discovers_symlinked_feature() {
    EXECUTIONS.fetch_add(1, Ordering::SeqCst);
}

scenarios!("tests/features/symlink_source");

#[test]
#[serial_test::serial]
fn macro_discovers_symlinked_directory() {
    const EXPECTED_EXECUTIONS: usize = 3;
    const ATTEMPTS: usize = 20;
    const WAIT_BETWEEN_ATTEMPTS: Duration = Duration::from_millis(50);

    for _ in 0..ATTEMPTS {
        if EXECUTIONS.load(Ordering::SeqCst) == EXPECTED_EXECUTIONS {
            return;
        }
        thread::sleep(WAIT_BETWEEN_ATTEMPTS);
    }

    panic!(
        "expected {EXPECTED_EXECUTIONS} step invocations, observed {}",
        EXECUTIONS.load(Ordering::SeqCst)
    );
}
