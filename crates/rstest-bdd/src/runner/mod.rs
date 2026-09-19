//! Parser-neutral scenario plans, outcomes, and the runners that execute them.
//!
//! A caller builds a [`ScenarioPlan`] — a name, tags, a source identity, and an
//! ordered list of step invocations — and hands it to a runner to obtain a
//! structured terminal [`ScenarioOutcome`]. Nothing in this module knows what a
//! `.feature` file is: the plan is built by whatever frontend parsed the source
//! text, and the runtime executes the same step definitions that the Gherkin
//! macros execute today.
//!
//! The module is deliberately free of `gherkin`, Markdown, Trymark, process,
//! snapshot, and reporter types, so a non-Gherkin frontend can keep its own
//! source paths and line numbers all the way into the outcome.
//!
//! # Building a plan
//!
//! ```
//! use rstest_bdd::{StepKeyword, runner::ScenarioPlanBuilder};
//!
//! let plan = ScenarioPlanBuilder::new("Add two numbers", "notes/arithmetic.md")
//!     .at_line(42)
//!     .step_at(StepKeyword::Given, "a calculator", 43)
//!     .step_at(StepKeyword::When, "I add 2 and 2", 44)
//!     .step_at(StepKeyword::Then, "the result is 4", 45)
//!     .build();
//!
//! assert_eq!(plan.name(), "Add two numbers");
//! assert_eq!(plan.steps().len(), 3);
//! let first = plan.steps().first().expect("three steps were added");
//! assert_eq!(first.source().map(|s| s.line()), Some(43));
//! ```

mod engine;
mod outcome;
mod plan;
mod source;

#[cfg(test)]
mod tests;

mod scope;

pub use outcome::{
    FailureKind,
    FailureSite,
    ScenarioFailure,
    ScenarioOutcome,
    ScenarioSkip,
    ScenarioStatus,
    StepOutcome,
    StepStatus,
    ValueFate,
};
pub use plan::{ScenarioPlan, StepInvocation, builder::ScenarioPlanBuilder};
pub use scope::{NoHooks, ScenarioScope};
pub use source::{SourceLocation, SourcePath};

/// Execute a plan synchronously and return its terminal outcome.
///
/// A failing step becomes part of the returned outcome. A value whose
/// destructor panics during cleanup is caught and logged as a warning rather
/// than returned: `ScenarioOutcome` carries exactly one failure channel
/// ([`failure`](ScenarioOutcome::failure) / `into_harness_result`), and a
/// second one would leave a caller unable to tell which failure was primary.
/// That is why `cleanup_error` was dropped — see D2 option (ii) and D13. A
/// *permitted* skip is reported as [`ScenarioStatus::Skipped`] rather than as a
/// failure, because whether a skip should fail a suite is the caller's policy
/// decision and [`ScenarioOutcome::into_harness_result`] is where it is made.
///
/// # Examples
///
/// ```
/// use rstest_bdd::{
///     StepContext,
///     StepKeyword,
///     runner::{ScenarioPlanBuilder, ScenarioScope, ScenarioStatus, run_scenario},
/// };
///
/// let mut ctx = StepContext::default();
/// let plan = ScenarioPlanBuilder::new("Add two numbers", "notes/arithmetic.md")
///     .step_at(StepKeyword::Given, "an undefined step", 3)
///     .build();
/// let outcome = run_scenario(&plan, ScenarioScope::new(&mut ctx));
///
/// // No step definition is registered in this doctest, so the run stops at the
/// // first invocation rather than unwinding.
/// assert_eq!(outcome.status(), ScenarioStatus::Failed);
/// ```
pub fn run_scenario<H>(
    plan: &ScenarioPlan,
    mut scope: ScenarioScope<'_, '_, H>,
) -> ScenarioOutcome {
    let (fail_on_skipped, ctx) = scope.split();
    engine::drive_sync::drive(plan, ctx, fail_on_skipped)
}

/// Execute a plan asynchronously and return its terminal outcome.
///
/// The asynchronous counterpart of [`run_scenario`], with the same contract for
/// everything a caller can observe: a failing step becomes part of the returned
/// outcome rather than unwinding, a permitted skip is reported as
/// [`ScenarioStatus::Skipped`] rather than as a failure, and
/// [`ScenarioOutcome::into_harness_result`] makes the policy decision about
/// whether a skip should fail a suite. For a plan whose every step definition is
/// registered in [`StepExecutionMode::Both`](crate::StepExecutionMode::Both),
/// the two runners produce equal outcomes; that is INV-5, and it is what
/// `crates/rstest-bdd/tests/runner_sequence_props.rs` asserts over generated
/// plans.
///
/// # Not `Send`
///
/// The returned future is **not** `Send`, because step scope guards are thread
/// bound. A caller cannot `tokio::spawn` it and must use a current-thread or
/// thread-per-scenario runtime. This is inherited from the step execution path
/// rather than chosen here, and it is why the guidance is a runtime *shape*
/// rather than "any runtime".
///
/// # Cancellation
///
/// Dropping the future cancels the run: no outcome is produced, and the
/// awaited after hook is not guaranteed to have run. Synchronous scope cleanup
/// still happens, because the future owns the scope — it is taken by value, so
/// dropping the future drops the scope and runs the cleanup guard.
///
/// # Examples
///
/// ```
/// use rstest_bdd::{
///     StepContext,
///     StepKeyword,
///     runner::{ScenarioPlanBuilder, ScenarioScope, ScenarioStatus, run_scenario_async},
/// };
///
/// let mut ctx = StepContext::default();
/// let plan = ScenarioPlanBuilder::new("Add two numbers", "notes/arithmetic.md")
///     .step_at(StepKeyword::Given, "an undefined step", 3)
///     .build();
///
/// let runtime = tokio::runtime::Builder::new_current_thread()
///     .build()
///     .expect("a current-thread runtime builds");
/// let outcome = runtime.block_on(run_scenario_async(&plan, ScenarioScope::new(&mut ctx)));
///
/// // No step definition is registered in this doctest, so the run stops at the
/// // first invocation rather than unwinding.
/// assert_eq!(outcome.status(), ScenarioStatus::Failed);
/// ```
pub async fn run_scenario_async<H>(
    plan: &ScenarioPlan,
    mut scope: ScenarioScope<'_, '_, H>,
) -> ScenarioOutcome {
    let (fail_on_skipped, ctx) = scope.split();
    engine::drive_async::drive(plan, ctx, fail_on_skipped).await
}
