//! Tests the environment-override and retry boundaries without mutating
//! process state.

use std::{
    any::Any,
    cell::{Cell, RefCell},
    env::VarError,
    ffi::OsString,
    panic::{self, AssertUnwindSafe},
};

use super::{
    RunSeedContext,
    TestDispatcher,
    calculate_seeds,
    parse_env_u64_value,
    run_seed,
    run_test,
};

thread_local! {
    /// Counts failure callbacks invoked on the current test thread.
    static ON_FAIL_CALLS: Cell<usize> = const { Cell::new(0) };
    /// Records every attempt's seed on the current test thread, in order.
    static SEED_LOG: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
}

/// Records one failure callback for the current test to count.
fn record_on_fail() { ON_FAIL_CALLS.with(|calls| calls.set(calls.get() + 1)); }

/// Returns how many failure callbacks ran on this test thread.
fn on_fail_call_count() -> usize { ON_FAIL_CALLS.with(Cell::get) }

/// Records one attempt and returns its zero-based index within its seed.
///
/// Counting per seed rather than per run lets a test fail the first attempt of
/// every seed while the run visits several of them.
fn record_seed(seed: u64) -> usize {
    SEED_LOG.with(|log| {
        let mut log = log.borrow_mut();
        let attempt = log.iter().filter(|seen| **seen == seed).count();
        log.push(seed);
        attempt
    })
}

/// Returns the seeds of every attempt on this test thread, in order.
fn recorded_seeds() -> Vec<u64> { SEED_LOG.with(|log| log.borrow().clone()) }

/// Clears the per-thread attempt record before a test body.
///
/// The counters are thread-local rather than process-wide, so tests stay
/// parallel-safe without serialising or mutating the environment.
fn reset_probes() {
    ON_FAIL_CALLS.with(|calls| calls.set(0));
    SEED_LOG.with(|log| log.borrow_mut().clear());
}

/// Returns the seeds [`run_test`] visits for one iteration and no explicit seed.
///
/// The expectation is derived rather than hard-coded because an ambient `SEED`
/// or `ITERATIONS` override changes the seed set. These tests must not mutate
/// the process environment to pin it, and a hard-coded seed count would fail
/// under such an override for reasons unrelated to the retry controller.
fn expected_seeds() -> Vec<u64> { calculate_seeds(1, &[]).0.collect() }

/// Returns the attempt sequence where every seed fails once and then succeeds.
fn fail_once_per_seed(seeds: &[u64]) -> Vec<u64> {
    seeds.iter().flat_map(|seed| [*seed; 2]).collect()
}

/// Runs one seed, panicking on the first `failures` attempts.
///
/// Returns the panic payload when the seed exhausts its retries, or `None`
/// when it recovers or succeeds.
fn run_seed_with_failures(
    seed: u64,
    max_retries: usize,
    failures: usize,
) -> Option<Box<dyn Any + Send>> {
    let mut test_fn = |dispatcher: TestDispatcher, seed: u64| {
        let attempt = record_seed(seed);
        assert_eq!(
            dispatcher.seed(),
            seed,
            "the dispatcher seed must match the seed passed to the closure"
        );
        assert!(attempt >= failures, "simulated attempt {attempt} failure");
    };

    let mut ctx = RunSeedContext {
        max_retries,
        test_fn: &mut test_fn,
        on_fail_fn: Some(record_on_fail),
        is_multiple_runs: false,
    };

    panic::catch_unwind(AssertUnwindSafe(|| run_seed(seed, &mut ctx))).err()
}

/// A seed that succeeds on its first attempt does not retry or report failure.
#[test]
fn succeeds_without_retrying_or_reporting_failure() {
    reset_probes();

    let payload = run_seed_with_failures(7, 3, 0);

    assert!(payload.is_none(), "a passing seed must not unwind");
    assert_eq!(
        recorded_seeds(),
        vec![7],
        "a passing seed must run exactly once"
    );
    assert_eq!(
        on_fail_call_count(),
        0,
        "a passing seed must not call on_fail_fn"
    );
}

/// A seed that fails once and then succeeds retries exactly once.
#[test]
fn retries_a_transient_failure() {
    reset_probes();

    let payload = run_seed_with_failures(11, 2, 1);

    assert!(payload.is_none(), "a recovering seed must not unwind");
    assert_eq!(
        recorded_seeds(),
        vec![11, 11],
        "one transient failure must add exactly one retry of the same seed"
    );
    assert_eq!(
        on_fail_call_count(),
        0,
        "a recovering seed must not call on_fail_fn"
    );
}

/// An exhausted seed reports failure once and resumes its final panic.
#[test]
fn exhausts_retries_and_resumes_the_final_panic() {
    reset_probes();
    let mut test_fn = |_dispatcher: TestDispatcher, seed: u64| {
        let attempt = record_seed(seed);
        panic!("attempt {attempt} failed");
    };
    let mut ctx = RunSeedContext {
        max_retries: 2,
        test_fn: &mut test_fn,
        on_fail_fn: Some(record_on_fail),
        is_multiple_runs: false,
    };

    let result = panic::catch_unwind(AssertUnwindSafe(|| run_seed(13, &mut ctx)));

    let Err(payload) = result else {
        panic!("an exhausted seed must resume its panic");
    };
    assert_eq!(
        recorded_seeds(),
        vec![13, 13, 13],
        "two retries must allow three attempts, all of the seed under test"
    );
    assert_eq!(
        on_fail_call_count(),
        1,
        "a terminal failure must call on_fail_fn exactly once"
    );
    assert_eq!(
        payload.downcast_ref::<String>().map(String::as_str),
        Some("attempt 2 failed"),
        "the final attempt's payload, not an earlier attempt's, must reach the caller"
    );
}

/// A zero retry budget allows one attempt and still reports the failure.
#[test]
fn zero_retry_budget_attempts_once_and_reports_the_failure() {
    reset_probes();
    let mut test_fn = |_dispatcher: TestDispatcher, seed: u64| {
        let attempt = record_seed(seed);
        panic!("attempt {attempt} failed");
    };
    let mut ctx = RunSeedContext {
        max_retries: 0,
        test_fn: &mut test_fn,
        on_fail_fn: Some(record_on_fail),
        is_multiple_runs: false,
    };

    let result = panic::catch_unwind(AssertUnwindSafe(|| run_seed(17, &mut ctx)));

    let Err(payload) = result else {
        panic!("a seed with no retry budget must resume its panic");
    };
    assert_eq!(
        recorded_seeds(),
        vec![17],
        "a budget of zero retries must allow exactly one attempt"
    );
    assert_eq!(
        on_fail_call_count(),
        1,
        "a budget of zero retries must still report the terminal failure once"
    );
    assert_eq!(
        payload.downcast_ref::<String>().map(String::as_str),
        Some("attempt 0 failed"),
        "the only attempt's payload must reach the caller unchanged"
    );
}

/// `run_test` forwards the retry budget to every seed it visits.
#[test]
fn run_test_retries_each_seed_within_the_forwarded_budget() {
    reset_probes();
    let seeds = expected_seeds();
    let mut test_fn = |dispatcher: TestDispatcher, seed: u64| {
        let attempt = record_seed(seed);
        assert_eq!(
            dispatcher.seed(),
            seed,
            "the dispatcher seed must match the seed passed to the closure"
        );
        assert!(attempt >= 1, "simulated transient failure");
    };

    run_test(1, &[], 1, &mut test_fn, Some(record_on_fail));

    assert!(!seeds.is_empty(), "run_test must visit at least one seed");
    assert_eq!(
        recorded_seeds(),
        fail_once_per_seed(&seeds),
        "a budget of one retry must give every seed exactly two consecutive attempts"
    );
    assert_eq!(
        on_fail_call_count(),
        0,
        "a recovering run must not call on_fail_fn"
    );
}

/// `run_test` reports one terminal failure and resumes the last attempt's panic.
#[test]
fn run_test_reports_one_failure_and_resumes_the_final_payload() {
    reset_probes();
    let seeds = expected_seeds();
    let Some(first_seed) = seeds.first().copied() else {
        panic!("run_test must visit at least one seed");
    };
    let mut test_fn = |_dispatcher: TestDispatcher, seed: u64| {
        let attempt = record_seed(seed);
        panic!("attempt {attempt} failed");
    };

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        run_test(1, &[], 2, &mut test_fn, Some(record_on_fail));
    }));

    let Err(payload) = result else {
        panic!("an exhausted run must resume its panic");
    };
    assert_eq!(
        recorded_seeds(),
        vec![first_seed; 3],
        "the first failing seed must run three times before the run unwinds"
    );
    assert_eq!(
        on_fail_call_count(),
        1,
        "run_test must forward on_fail_fn to exactly one terminal failure"
    );
    assert_eq!(
        payload.downcast_ref::<String>().map(String::as_str),
        Some("attempt 2 failed"),
        "the final attempt's payload, not an earlier attempt's, must reach the caller"
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
