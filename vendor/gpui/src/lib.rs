//! Stable-compatible GPUI test support shim used by the rstest-bdd workspace.
//!
//! This crate intentionally implements only the GPUI test support surface that
//! rstest-bdd uses: `#[gpui::test]`, `run_test`, `TestDispatcher`, and
//! `TestAppContext`.

use std::{
    any::Any,
    env,
    future::Future,
    panic::{self, RefUnwindSafe},
    process::Termination,
    sync::Arc,
};

pub use gpui_macros::test;

enum AttemptAction {
    Success,
    Retry,
    ResumeUnwind(Box<dyn Any + Send>),
}

impl AttemptAction {
    const fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }

    const fn is_retry(&self) -> bool {
        matches!(self, Self::Retry)
    }

    fn into_panic(self) -> Box<dyn Any + Send> {
        match self {
            Self::ResumeUnwind(error) => error,
            Self::Success | Self::Retry => unreachable!("only failure actions carry panic data"),
        }
    }
}

/// Minimal dispatcher value passed through `run_test`.
#[derive(Clone, Debug, Default)]
pub struct TestDispatcher {
    seed: u64,
}

impl TestDispatcher {
    /// Creates a dispatcher for the supplied deterministic seed.
    #[must_use]
    pub const fn new(seed: u64) -> Self { Self { seed } }

    /// Returns the deterministic seed associated with this dispatcher.
    #[must_use]
    pub const fn seed(&self) -> u64 { self.seed }

    /// Drives queued work until the dispatcher is idle.
    pub fn run_until_parked(&self) {}
}

/// Minimal executor used by the test shim.
#[derive(Clone, Debug, Default)]
pub struct BackgroundExecutor;

impl BackgroundExecutor {
    /// Creates an executor associated with the supplied dispatcher.
    #[must_use]
    pub fn new(_dispatcher: Arc<TestDispatcher>) -> Self { Self }

    /// Blocks on an async test future.
    pub fn block_test<F>(&self, future: F) -> F::Output
    where
        F: Future,
    {
        futures::executor::block_on(future)
    }

    /// Prevents additional parking in upstream GPUI; a no-op here.
    pub fn forbid_parking(&self) {}
}

/// Lightweight test context made available to GPUI-backed scenario execution.
#[derive(Clone, Debug)]
pub struct TestAppContext {
    dispatcher: TestDispatcher,
    executor: BackgroundExecutor,
    fn_name: Option<&'static str>,
}

impl TestAppContext {
    /// Builds a new test context from the supplied dispatcher and function name.
    #[must_use]
    pub fn build(dispatcher: TestDispatcher, fn_name: Option<&'static str>) -> Self {
        Self {
            dispatcher: dispatcher.clone(),
            executor: BackgroundExecutor::new(Arc::new(dispatcher)),
            fn_name,
        }
    }

    /// Creates a single-context instance seeded with `0`.
    #[must_use]
    pub fn single() -> Self { Self::build(TestDispatcher::new(0), None) }

    /// Returns the originating test function name when available.
    #[must_use]
    pub const fn test_function_name(&self) -> Option<&'static str> { self.fn_name }

    /// Returns the dispatcher associated with this context.
    #[must_use]
    pub const fn dispatcher(&self) -> &TestDispatcher { &self.dispatcher }

    /// Returns the background executor associated with this context.
    #[must_use]
    pub fn executor(&self) -> BackgroundExecutor { self.executor.clone() }

    /// Reports whether a path prompt was observed during the test run.
    #[must_use]
    pub const fn did_prompt_for_new_path(&self) -> bool { false }

    /// Registers cleanup to run when the test context shuts down.
    pub fn on_quit(&mut self, _callback: impl FnOnce() + 'static) {}

    /// Tears down the test context. This shim has no extra cleanup.
    pub fn quit(&self) {}
}

/// Panics when a GPUI test result reports failure.
#[doc(hidden)]
pub fn assert_test_outcome<T>(result: T)
where
    T: Termination,
{
    let exit_code = result.report();
    assert_eq!(
        exit_code,
        std::process::ExitCode::SUCCESS,
        "gpui::test reported failure"
    );
}

/// Runs a GPUI-style test closure for the supplied seeds and retry policy.
pub fn run_test(
    num_iterations: usize,
    explicit_seeds: &[u64],
    max_retries: usize,
    test_fn: &mut (dyn RefUnwindSafe + Fn(TestDispatcher, u64)),
    on_fail_fn: Option<fn()>,
) {
    let (seeds, is_multiple_runs) = calculate_seeds(num_iterations as u64, explicit_seeds);

    for seed in seeds {
        let mut attempt = 0;
        loop {
            if is_multiple_runs {
                eprintln!("seed = {seed}");
            }

            let action = classify_attempt(seed, attempt, max_retries, test_fn);
            if action.is_success() {
                break;
            }
            if action.is_retry() {
                println!("attempt {attempt} failed, retrying");
                attempt += 1;
                continue;
            }

            if is_multiple_runs {
                eprintln!("failing seed: {seed}");
            }
            if let Some(on_fail_fn) = on_fail_fn {
                on_fail_fn();
            }
            panic::resume_unwind(action.into_panic());
        }
    }
}

fn classify_attempt(
    seed: u64,
    attempt: usize,
    max_retries: usize,
    test_fn: &(dyn RefUnwindSafe + Fn(TestDispatcher, u64)),
) -> AttemptAction {
    let result = panic::catch_unwind(|| {
        let dispatcher = TestDispatcher::new(seed);
        test_fn(dispatcher, seed);
    });

    match result {
        Ok(()) => AttemptAction::Success,
        Err(_) if attempt < max_retries => AttemptAction::Retry,
        Err(error) => AttemptAction::ResumeUnwind(error),
    }
}

fn calculate_seeds(
    iterations: u64,
    explicit_seeds: &[u64],
) -> (impl Iterator<Item = u64> + '_, bool) {
    let iterations = resolve_iterations(iterations);
    let env_seed = parse_env_u64("SEED");
    let seeds = select_seeds(iterations, explicit_seeds, env_seed);

    let is_multiple_runs = seeds.len() > 1;
    (seeds.into_iter(), is_multiple_runs)
}

fn resolve_iterations(default_iterations: u64) -> u64 {
    parse_env_u64("ITERATIONS")
        .unwrap_or(default_iterations)
        .max(1)
}

fn parse_env_u64(key: &str) -> Option<u64> {
    let value = env::var(key).ok()?;
    match value.parse() {
        Ok(parsed) => Some(parsed),
        Err(error) => {
            eprintln!("ignoring invalid {key} value {value:?}: {error}");
            None
        }
    }
}

fn select_seeds(iterations: u64, explicit_seeds: &[u64], env_seed: Option<u64>) -> Vec<u64> {
    if let Some(seed) = env_seed {
        return build_seed_range(seed, iterations);
    }

    let mut seeds = build_seed_range(0, iterations);
    if iterations == 1 && explicit_seeds.is_empty() {
        return seeds;
    }

    seeds.extend(explicit_seeds.iter().copied());
    seeds
}

fn build_seed_range(start: u64, iterations: u64) -> Vec<u64> {
    let mut seeds = Vec::new();
    for offset in 0..iterations.max(1) {
        match start.checked_add(offset) {
            Some(seed) => seeds.push(seed),
            None => {
                eprintln!(
                    "seed range overflowed for start={start} and iterations={iterations}; \
                     truncating generated seeds"
                );
                break;
            }
        }
    }

    if seeds.is_empty() {
        seeds.push(start);
    }

    seeds
}
