//! Stable-compatible GPUI test support shim used by the rstest-bdd workspace.
//!
//! This crate intentionally implements only the GPUI test support surface that
//! rstest-bdd uses: `#[gpui::test]`, `run_test`, `TestDispatcher`, and
//! `TestAppContext`.

use std::{
    env,
    future::Future,
    panic::{self, RefUnwindSafe},
    sync::Arc,
};

pub use gpui_macros::test;

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

            let result = panic::catch_unwind(|| {
                let dispatcher = TestDispatcher::new(seed);
                test_fn(dispatcher, seed);
            });

            match result {
                Ok(()) => break,
                Err(error) if attempt < max_retries => {
                    println!("attempt {attempt} failed, retrying");
                    attempt += 1;
                    std::mem::forget(error);
                }
                Err(error) => {
                    if is_multiple_runs {
                        eprintln!("failing seed: {seed}");
                    }
                    if let Some(on_fail_fn) = on_fail_fn {
                        on_fail_fn();
                    }
                    panic::resume_unwind(error);
                }
            }
        }
    }
}

fn calculate_seeds(
    iterations: u64,
    explicit_seeds: &[u64],
) -> (impl Iterator<Item = u64> + '_, bool) {
    let iterations = env::var("ITERATIONS")
        .ok()
        .map(|value| value.parse().expect("invalid ITERATIONS variable"))
        .unwrap_or(iterations);

    let env_seed = env::var("SEED")
        .ok()
        .map(|value| value.parse().expect("invalid SEED variable as integer"));

    let seeds = match (iterations, env_seed) {
        (_, Some(seed)) => (seed..seed + iterations.max(1)).collect::<Vec<_>>(),
        (1, None) if explicit_seeds.is_empty() => vec![0],
        (_, None) => (0..iterations).chain(explicit_seeds.iter().copied()).collect(),
    };

    let is_multiple_runs = seeds.len() > 1;
    (seeds.into_iter(), is_multiple_runs)
}
