//! Tests the environment-override and retry boundaries without mutating
//! process state.

use std::{
    any::Any,
    cell::Cell,
    env::VarError,
    ffi::OsString,
    panic::{self, AssertUnwindSafe},
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};

use super::{SeedRun, TestDispatcher, parse_env_u64_value, run_seed};

thread_local! {
    /// Counts failure callbacks invoked on the current test thread.
    static ON_FAIL_CALLS: Cell<usize> = const { Cell::new(0) };
}

/// Records one failure callback for the current test to count.
fn record_on_fail() { ON_FAIL_CALLS.with(|calls| calls.set(calls.get() + 1)); }

/// Returns how many failure callbacks ran on this test thread.
fn on_fail_call_count() -> usize { ON_FAIL_CALLS.with(Cell::get) }

/// Resets the per-thread failure-callback count before a test body.
fn reset_on_fail_calls() { ON_FAIL_CALLS.with(|calls| calls.set(0)); }

/// Observes each attempt's seed and counts how many attempts ran.
#[derive(Default)]
struct RetryProbe {
    /// Number of attempts recorded so far.
    attempts: AtomicUsize,
    /// Seed reported by the first attempt.
    first_seed: AtomicU64,
    /// Non-zero when a later attempt saw a different seed.
    seed_changed: AtomicUsize,
}

impl RetryProbe {
    /// Records one attempt and returns its zero-based index.
    fn record(&self, seed: u64) -> usize {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
        if attempt == 0 {
            self.first_seed.store(seed, Ordering::SeqCst);
        } else if self.first_seed.load(Ordering::SeqCst) != seed {
            self.seed_changed.store(1, Ordering::SeqCst);
        }
        attempt
    }

    /// Returns the number of attempts observed.
    fn attempts(&self) -> usize { self.attempts.load(Ordering::SeqCst) }

    /// Returns the seed the first attempt received.
    fn first_seed(&self) -> u64 { self.first_seed.load(Ordering::SeqCst) }

    /// Reports whether any retry received a different seed.
    fn seed_changed(&self) -> bool { self.seed_changed.load(Ordering::SeqCst) != 0 }
}

/// Builds the initial state of a single-seed run.
const fn first_attempt(seed: u64, max_retries: usize) -> SeedRun {
    SeedRun {
        seed,
        is_multiple_runs: false,
        max_retries,
        attempt: 0,
    }
}

/// Runs one seed, panicking on the first `failures` attempts.
///
/// Returns the panic payload when the seed exhausts its retries, or `None`
/// when it recovers or succeeds.
fn run_with_failures(
    seed_run: SeedRun,
    failures: usize,
    probe: &RetryProbe,
) -> Option<Box<dyn Any + Send>> {
    let mut test_fn = |dispatcher: TestDispatcher, seed: u64| {
        let attempt = probe.record(seed);
        assert_eq!(
            dispatcher.seed(),
            seed,
            "the dispatcher seed must match the seed passed to the closure"
        );
        assert!(attempt >= failures, "simulated attempt {attempt} failure");
    };

    panic::catch_unwind(AssertUnwindSafe(|| {
        run_seed(seed_run, &mut test_fn, Some(record_on_fail));
    }))
    .err()
}

/// A seed that succeeds on its first attempt does not retry or report failure.
#[test]
fn succeeds_without_retrying_or_reporting_failure() {
    reset_on_fail_calls();
    let probe = RetryProbe::default();

    let payload = run_with_failures(first_attempt(7, 3), 0, &probe);

    assert!(payload.is_none(), "a passing seed must not unwind");
    assert_eq!(probe.attempts(), 1, "a passing seed must run exactly once");
    assert_eq!(
        on_fail_call_count(),
        0,
        "a passing seed must not call on_fail_fn"
    );
}

/// A seed that fails once and then succeeds retries exactly once.
#[test]
fn retries_a_transient_failure() {
    reset_on_fail_calls();
    let probe = RetryProbe::default();

    let payload = run_with_failures(first_attempt(11, 2), 1, &probe);

    assert!(payload.is_none(), "a recovering seed must not unwind");
    assert_eq!(
        probe.attempts(),
        2,
        "one transient failure must add exactly one retry"
    );
    assert_eq!(
        on_fail_call_count(),
        0,
        "a recovering seed must not call on_fail_fn"
    );
    assert_eq!(
        probe.first_seed(),
        11,
        "every attempt must receive the seed under test"
    );
    assert!(
        !probe.seed_changed(),
        "retries must reuse the seed from the first attempt"
    );
}

/// An exhausted seed reports failure once and resumes its original panic.
#[test]
fn exhausts_retries_and_resumes_the_original_panic() {
    reset_on_fail_calls();
    let probe = RetryProbe::default();
    let mut test_fn = |_dispatcher: TestDispatcher, seed: u64| {
        probe.record(seed);
        panic!("terminal failure payload");
    };

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        run_seed(first_attempt(13, 2), &mut test_fn, Some(record_on_fail));
    }));

    let Err(payload) = result else {
        panic!("an exhausted seed must resume its panic");
    };
    assert_eq!(
        probe.attempts(),
        3,
        "two retries must allow three attempts in total"
    );
    assert_eq!(
        on_fail_call_count(),
        1,
        "a terminal failure must call on_fail_fn exactly once"
    );
    assert_eq!(
        probe.first_seed(),
        13,
        "the terminal run must keep the seed under test"
    );
    assert!(
        !probe.seed_changed(),
        "retries must reuse the seed from the first attempt"
    );
    assert_eq!(
        payload.downcast_ref::<&str>().copied(),
        Some("terminal failure payload"),
        "the original panic payload must reach the caller unchanged"
    );
}

/// Treats an injected non-Unicode lookup failure as an invalid override.
#[test]
fn discards_non_unicode_environment_override() {
    let value = VarError::NotUnicode(OsString::from("invalid-byte-sequence"));

    assert_eq!(
        parse_env_u64_value("SEED", Err(value)),
        None,
        "a non-Unicode override must be discarded"
    );
}
