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
/// Never panics: a failing step and a returned value's failing destructor both
/// become part of the returned outcome. A *permitted* skip is reported as
/// [`ScenarioStatus::Skipped`] rather than as a failure, because whether a skip
/// should fail a suite is the caller's policy decision and
/// [`ScenarioOutcome::into_harness_result`] is where it is made.
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
