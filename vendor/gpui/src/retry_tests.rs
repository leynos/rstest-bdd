//! Tests the environment-override and retry boundaries without mutating
//! process state.

use std::{
    cell::{Cell, RefCell},
    panic::{self, AssertUnwindSafe},
};

use super::{
    RunSeedContext,
    TestDispatcher,
    calculate_seeds_with,
    parse_env_u64,
    run_seed,
    run_test,
    run_test_with,
};

thread_local! {
    /// Counts failure callbacks invoked on the current test thread.
    static ON_FAIL_CALLS: Cell<usize> = const { Cell::new(0) };
    /// Records every attempt's seed on the current test thread, in order.
    static SEED_LOG: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
    /// Tracks one seed at a time for the public all-seed retry test.
    static PUBLIC_CURRENT_SEED: Cell<Option<u64>> = const { Cell::new(None) };
    /// Counts attempts for the current seed in the public retry test.
    static PUBLIC_SEED_ATTEMPTS: Cell<usize> = const { Cell::new(0) };
    /// Counts selected seeds without retaining a log of the full run.
    static PUBLIC_SELECTED_SEEDS: Cell<usize> = const { Cell::new(0) };
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

/// Records one public-entry attempt using constant space.
fn record_public_seed_attempt(seed: u64) -> usize {
    let previous_seed = PUBLIC_CURRENT_SEED.with(Cell::get);
    if previous_seed != Some(seed) {
        if previous_seed.is_some() {
            assert_eq!(
                PUBLIC_SEED_ATTEMPTS.with(Cell::get),
                2,
                "each completed public seed must have exactly one retry"
            );
        }
        PUBLIC_CURRENT_SEED.with(|current_seed| current_seed.set(Some(seed)));
        PUBLIC_SEED_ATTEMPTS.with(|attempts| attempts.set(0));
        PUBLIC_SELECTED_SEEDS.with(|count| count.set(count.get().saturating_add(1)));
    }
    let attempts =
        PUBLIC_SEED_ATTEMPTS.with(|current_attempts| current_attempts.get().saturating_add(1));
    PUBLIC_SEED_ATTEMPTS.with(|current_attempts| current_attempts.set(attempts));
    attempts
}

/// Returns the selected-seed count and final seed's attempt count.
fn public_seed_summary() -> (usize, usize) {
    (
        PUBLIC_SELECTED_SEEDS.with(Cell::get),
        PUBLIC_SEED_ATTEMPTS.with(Cell::get),
    )
}

/// Returns the seeds of every attempt on this test thread, in order.
fn recorded_seeds() -> Vec<u64> { SEED_LOG.with(|log| log.borrow().clone()) }

/// Clears the per-thread attempt record before a test body.
///
/// The counters are thread-local rather than process-wide, so tests stay
/// parallel-safe without serializing or mutating the environment.
fn reset_probes() {
    ON_FAIL_CALLS.with(|calls| calls.set(0));
    SEED_LOG.with(|log| log.borrow_mut().clear());
    PUBLIC_CURRENT_SEED.with(|seed| seed.set(None));
    PUBLIC_SEED_ATTEMPTS.with(|attempts| attempts.set(0));
    PUBLIC_SELECTED_SEEDS.with(|count| count.set(0));
}

/// One expected single-seed outcome for a given budget and failure count.
struct SeedCase {
    /// The seed every attempt of the case carries.
    seed: u64,
    /// How many retries the budget allows.
    max_retries: usize,
    /// How many attempts fail before the closure returns.
    failures: usize,
    /// The payload expected when the seed fails terminally.
    payload: Option<&'static str>,
    /// The seeds expected in attempt order.
    seeds: &'static [u64],
    /// How many failure callbacks the case expects.
    on_fail_calls: usize,
}

/// What one single-seed run left behind for a test to assert on.
struct SeedOutcome {
    /// The resumed panic payload, rendered, when the seed failed terminally.
    payload: Option<String>,
    /// Every attempt's seed, in order.
    seeds: Vec<u64>,
    /// How many failure callbacks ran.
    on_fail_calls: usize,
}

/// Runs one seed, panicking on its first `failures` attempts, and records the
/// observable result.
fn observe_seed(seed: u64, max_retries: usize, failures: usize) -> SeedOutcome {
    reset_probes();
    let mut test_fn = |dispatcher: TestDispatcher, seed: u64| {
        let attempt = record_seed(seed);
        assert_eq!(
            dispatcher.seed(),
            seed,
            "the dispatcher seed must match the seed passed to the closure"
        );
        assert!(attempt >= failures, "attempt {attempt} failed");
    };
    let mut ctx = RunSeedContext {
        max_retries,
        test_fn: &mut test_fn,
        on_fail_fn: Some(record_on_fail),
        is_multiple_runs: false,
    };

    let payload = panic::catch_unwind(AssertUnwindSafe(|| run_seed(seed, &mut ctx)))
        .err()
        .map(|payload| {
            payload
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_else(|| "<non-string payload>".to_owned())
        });
    SeedOutcome {
        payload,
        seeds: recorded_seeds(),
        on_fail_calls: on_fail_call_count(),
    }
}

/// The retry controller's per-seed outcomes across budgets and failure counts.
///
/// The rows are the boundaries the controller distinguishes: a first-attempt
/// pass, a transient failure that recovers, an exhausted budget that resumes
/// its final payload, and a zero budget that must still attempt once.
#[test]
fn reports_the_expected_outcome_for_each_retry_budget() {
    let cases = [
        SeedCase {
            seed: 7,
            max_retries: 3,
            failures: 0,
            payload: None,
            seeds: &[7],
            on_fail_calls: 0,
        },
        SeedCase {
            seed: 11,
            max_retries: 2,
            failures: 1,
            payload: None,
            seeds: &[11, 11],
            on_fail_calls: 0,
        },
        SeedCase {
            seed: 13,
            max_retries: 2,
            failures: usize::MAX,
            payload: Some("attempt 2 failed"),
            seeds: &[13, 13, 13],
            on_fail_calls: 1,
        },
        SeedCase {
            seed: 17,
            max_retries: 0,
            failures: usize::MAX,
            payload: Some("attempt 0 failed"),
            seeds: &[17],
            on_fail_calls: 1,
        },
    ];

    for case in cases {
        let outcome = observe_seed(case.seed, case.max_retries, case.failures);
        assert_eq!(
            outcome.payload.as_deref(),
            case.payload,
            "seed {} with {} retries must resume the final attempt's payload",
            case.seed,
            case.max_retries
        );
        assert_eq!(
            outcome.seeds, case.seeds,
            "seed {} with {} retries must attempt exactly the listed seeds",
            case.seed, case.max_retries
        );
        assert_eq!(
            outcome.on_fail_calls, case.on_fail_calls,
            "seed {} with {} retries must report failure the listed number of times",
            case.seed, case.max_retries
        );
    }
}

/// The injected entry point forwards the retry budget to every seed it visits.
///
/// Two iterations give two distinct seeds and, with the injected lookup
/// answering nothing, no ambient `SEED` or `ITERATIONS` value can change them.
#[test]
fn run_test_retries_each_seed_within_the_forwarded_budget() {
    reset_probes();
    let mut test_fn = |dispatcher: TestDispatcher, seed: u64| {
        let attempt = record_seed(seed);
        assert_eq!(
            dispatcher.seed(),
            seed,
            "the dispatcher seed must match the seed passed to the closure"
        );
        assert!(attempt >= 1, "simulated transient failure");
    };

    run_test_with(2, &[], 1, &mut test_fn, Some(record_on_fail), |_key| None);

    assert_eq!(
        recorded_seeds(),
        vec![0, 0, 1, 1],
        "a budget of one retry must give each of the two seeds two consecutive attempts"
    );
    assert_eq!(
        on_fail_call_count(),
        0,
        "a recovering run must not call on_fail_fn"
    );
}

/// The public entry point forwards the requested iteration and retry counts.
///
/// The expectation comes from the same lazy resolver the entry point uses, so
/// ambient overrides change both sides together. Thread-local counters keep
/// the per-seed checks constant-space while the closure remains `Fn`.
#[test]
fn public_run_test_retries_each_selected_seed() {
    reset_probes();
    let requested_iterations = 2;
    let expected_seed_count = calculate_seeds_with(
        u64::try_from(requested_iterations).expect("two iterations must fit in u64"),
        &[],
        parse_env_u64,
    )
    .0
    .size_hint()
    .0;
    let mut test_fn = |_dispatcher: TestDispatcher, seed: u64| {
        let attempt = record_public_seed_attempt(seed);
        assert!(attempt != 1, "simulated transient failure for seed {seed}");
        assert_eq!(
            attempt, 2,
            "every selected seed must succeed on its single retry"
        );
    };

    run_test(
        requested_iterations,
        &[],
        1,
        &mut test_fn,
        Some(record_on_fail),
    );

    let (selected_seed_count, final_seed_attempts) = public_seed_summary();
    assert_eq!(
        selected_seed_count, expected_seed_count,
        "the public entry point must visit each selected seed twice under a budget of one"
    );
    assert_eq!(
        final_seed_attempts, 2,
        "the final selected seed must also receive exactly one retry"
    );
    assert_eq!(
        on_fail_call_count(),
        0,
        "a recovering run must not call on_fail_fn through the public entry point"
    );
}

/// The public entry point calls the failure hook and resumes the final payload.
#[test]
fn public_run_test_reports_terminal_failure_and_resumes_final_payload() {
    reset_probes();
    let mut test_fn = |_dispatcher: TestDispatcher, seed: u64| {
        let attempt = record_seed(seed);
        panic!("attempt {attempt} failed for seed {seed}");
    };

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        run_test(2, &[], 1, &mut test_fn, Some(record_on_fail));
    }));

    let Err(payload) = result else {
        panic!("an exhausted public run must resume its panic");
    };
    let seeds = recorded_seeds();
    assert_eq!(
        seeds.len(),
        2,
        "one retry must make exactly two attempts before a terminal failure"
    );
    let [first_seed, second_seed] = seeds.as_slice() else {
        panic!("two attempts must be recorded before the terminal failure");
    };
    assert_eq!(
        first_seed, second_seed,
        "both terminal attempts must use the same selected seed"
    );
    assert_eq!(
        on_fail_call_count(),
        1,
        "the public entry point must call on_fail_fn exactly once"
    );
    let expected_payload = format!("attempt 1 failed for seed {first_seed}");
    assert_eq!(
        payload.downcast_ref::<String>().map(String::as_str),
        Some(expected_payload.as_str()),
        "the public entry point must resume the final attempt's panic payload"
    );
}

/// The injected entry point reports one terminal failure and resumes the final
/// attempt's panic.
#[test]
fn run_test_reports_one_failure_and_resumes_the_final_payload() {
    reset_probes();
    let mut test_fn = |_dispatcher: TestDispatcher, seed: u64| {
        let attempt = record_seed(seed);
        panic!("attempt {attempt} failed");
    };

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        run_test_with(2, &[], 2, &mut test_fn, Some(record_on_fail), |_key| None);
    }));

    let Err(payload) = result else {
        panic!("an exhausted run must resume its panic");
    };
    assert_eq!(
        recorded_seeds(),
        vec![0, 0, 0],
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
