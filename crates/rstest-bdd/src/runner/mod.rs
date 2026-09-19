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

mod outcome;
mod plan;
mod source;

#[cfg(test)]
mod tests;

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
pub use source::{SourceLocation, SourcePath};
