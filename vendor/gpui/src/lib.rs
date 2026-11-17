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
pub use test_window::{AnyWindowHandle, Entity, EntityError, VisualTestContext};

mod test_window;

/// Classifies one protected test invocation for the retry controller.
enum AttemptAction {
    /// The test body completed without unwinding.
    Success,
    /// The panic is eligible for another attempt.
    Retry,
    /// The final panic payload must resume after the failure callback runs.
    ResumeUnwind(Box<dyn Any + Send>),
}

/// Per-seed inputs for this vendored GPUI retry controller.
///
/// Keep this value local to the shim; it is not a general test-run API.
#[derive(Clone, Copy)]
struct SeedRun {
    /// Seed passed to the test closure.
    seed: u64,
    /// Whether the invocation contains multiple seeds.
    is_multiple_runs: bool,
    /// Maximum number of retries allowed after the first attempt.
    max_retries: usize,
    /// Current zero-based attempt, where zero is the first run.
    attempt: usize,
}

/// Minimal dispatcher value passed through `run_test`.
#[derive(Clone, Debug, Default)]
pub struct TestDispatcher {
    /// Deterministic input used to identify the active test attempt.
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
    /// Dispatcher retained so views and generated teardown share the attempt seed.
    dispatcher: TestDispatcher,
    /// Executor used when an expanded test function is asynchronous.
    executor: BackgroundExecutor,
    /// Original test name, retained for assertions made inside generated contexts.
    fn_name: Option<&'static str>,
    /// Windows and entities owned exclusively by this test context.
    windows: test_window::WindowRegistry,
}

impl TestAppContext {
    /// Builds a new test context from the supplied dispatcher and function name.
    #[must_use]
    pub fn build(dispatcher: TestDispatcher, fn_name: Option<&'static str>) -> Self {
        Self {
            dispatcher: dispatcher.clone(),
            executor: BackgroundExecutor::new(Arc::new(dispatcher)),
            fn_name,
            windows: test_window::WindowRegistry::default(),
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

    /// Creates a test window containing the view returned by `build_view`.
    pub fn add_window_view<T>(
        &mut self,
        build_view: impl FnOnce(&mut VisualTestContext) -> T,
    ) -> (Entity<T>, VisualTestContext)
    where
        T: 'static,
    {
        self.windows.add_window_view(build_view)
    }

    /// Returns the window handles known to this test context.
    #[must_use]
    pub fn windows(&self) -> Vec<AnyWindowHandle> { self.windows.handles() }

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
///
/// The retry diagnostics deliberately go directly to the test process because
/// this minimal shim does not install a global tracing subscriber.
pub fn run_test(
    num_iterations: usize,
    explicit_seeds: &[u64],
    max_retries: usize,
    test_fn: &mut (dyn RefUnwindSafe + Fn(TestDispatcher, u64)),
    on_fail_fn: Option<fn()>,
) {
    let (seeds, is_multiple_runs) = calculate_seeds(num_iterations as u64, explicit_seeds);

    for seed in seeds {
        run_seed(
            SeedRun {
                seed,
                is_multiple_runs,
                max_retries,
                attempt: 0,
            },
            test_fn,
            on_fail_fn,
        );
    }
}

/// Runs one seed until it succeeds or exhausts the configured retries.
fn run_seed(
    seed_run: SeedRun,
    test_fn: &mut (dyn RefUnwindSafe + Fn(TestDispatcher, u64)),
    on_fail_fn: Option<fn()>,
) {
    for attempt in 0..=seed_run.max_retries {
        let seed_run = SeedRun {
            attempt,
            ..seed_run
        };
        report_seed(seed_run);
        match classify_attempt(seed_run, test_fn) {
            AttemptAction::Success => return,
            AttemptAction::Retry => report_retry(seed_run),
            AttemptAction::ResumeUnwind(payload) => {
                resume_failed_seed(seed_run, on_fail_fn, payload);
            }
        }
    }

    unreachable!("the final retry attempt must succeed or resume its panic");
}

/// Prints a seed label when the test runs with multiple seeds.
#[expect(
    clippy::print_stderr,
    reason = "seed diagnostics must remain visible without a global tracing subscriber"
)]
fn report_seed(seed_run: SeedRun) {
    if seed_run.is_multiple_runs {
        eprintln!("seed = {}", seed_run.seed);
    }
}

/// Prints progress after a failed attempt that will be retried.
#[expect(
    clippy::print_stdout,
    reason = "retry progress must remain visible without a global tracing subscriber"
)]
fn report_retry(seed_run: SeedRun) {
    println!("attempt {} failed, retrying", seed_run.attempt);
}

/// Reports a terminal seed failure and resumes its original panic payload.
fn resume_failed_seed(
    seed_run: SeedRun,
    on_fail_fn: Option<fn()>,
    payload: Box<dyn Any + Send>,
) -> ! {
    report_failing_seed(seed_run);
    if let Some(on_fail_fn) = on_fail_fn {
        on_fail_fn();
    }
    panic::resume_unwind(payload)
}

/// Prints a seed label when a multi-seed run reaches a terminal failure.
#[expect(
    clippy::print_stderr,
    reason = "the failing seed must remain visible without a global tracing subscriber"
)]
fn report_failing_seed(seed_run: SeedRun) {
    if seed_run.is_multiple_runs {
        eprintln!("failing seed: {}", seed_run.seed);
    }
}

/// Run one attempt under unwind capture and classify the retry decision.
///
/// Only the final failure retains its payload, so callers can run their
/// callback before resuming the original panic without changing its value.
fn classify_attempt(
    seed_run: SeedRun,
    test_fn: &(dyn RefUnwindSafe + Fn(TestDispatcher, u64)),
) -> AttemptAction {
    let result = panic::catch_unwind(|| {
        let dispatcher = TestDispatcher::new(seed_run.seed);
        test_fn(dispatcher, seed_run.seed);
    });

    match result {
        Ok(()) => AttemptAction::Success,
        Err(_) if seed_run.attempt < seed_run.max_retries => AttemptAction::Retry,
        Err(error) => AttemptAction::ResumeUnwind(error),
    }
}

/// Resolve the complete seed sequence and whether seed labels aid diagnostics.
///
/// Environment overrides take precedence over explicit seeds to mirror GPUI's
/// reproducible test-run contract.
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

/// Resolve the iteration count while preserving at least one test execution.
fn resolve_iterations(default_iterations: u64) -> u64 {
    parse_env_u64("ITERATIONS")
        .unwrap_or(default_iterations)
        .max(1)
}

/// Parse an optional unsigned environment override without failing the test run.
///
/// Invalid values are diagnosed to stderr because this shim has no logging
/// subscriber and must make a discarded override observable to the developer.
fn parse_env_u64(key: &str) -> Option<u64> { parse_env_u64_value(key, env::var(key)) }

/// Parse a fallible environment lookup as an optional unsigned override.
///
/// A missing variable is deliberately optional, whereas an invalid Unicode or
/// numeric value is discarded after its diagnostic makes the configuration
/// mistake visible to the test author.
#[expect(
    clippy::print_stderr,
    reason = "invalid environment overrides must be visible without global logging"
)]
fn parse_env_u64_value(key: &str, value: Result<String, env::VarError>) -> Option<u64> {
    match value {
        Ok(value) => match value.parse() {
            Ok(parsed) => Some(parsed),
            Err(error) => {
                eprintln!("ignoring invalid {key} value {value:?}: {error}");
                None
            }
        },
        Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(value)) => {
            eprintln!(
                "ignoring invalid {key} value {}: value is not valid Unicode",
                value.display()
            );
            None
        }
    }
}

/// Select deterministic seeds, giving an environment override precedence.
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

/// Build a non-empty consecutive seed range, truncating safely on overflow.
///
/// The truncation warning is emitted directly because silently wrapping would
/// make a failing run unreproducible.
#[expect(
    clippy::print_stderr,
    reason = "seed-range truncation must be visible without global logging"
)]
fn build_seed_range(start: u64, iterations: u64) -> Vec<u64> {
    let mut seeds = Vec::new();
    for offset in 0..iterations.max(1) {
        if let Some(seed) = start.checked_add(offset) {
            seeds.push(seed);
        } else {
            eprintln!(
                "seed range overflowed for start={start} and iterations={iterations}; truncating \
                 generated seeds"
            );
            break;
        }
    }

    if seeds.is_empty() {
        seeds.push(start);
    }

    seeds
}

#[cfg(test)]
mod retry_tests;
