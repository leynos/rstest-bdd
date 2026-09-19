//! Parser-neutral scenario plans, outcomes, and the runners that execute them.
//!
//! A caller builds a [`ScenarioPlan`](plan::ScenarioPlan) — a name, tags, a
//! source identity, and an ordered list of step invocations — and hands it to a
//! runner to obtain a structured terminal
//! [`ScenarioOutcome`](outcome::ScenarioOutcome). Nothing in this module knows
//! what a `.feature` file is: the plan is built by whatever frontend parsed the
//! source text, and the runtime executes the same step definitions that the
//! Gherkin macros execute today.
//!
//! The module is deliberately free of `gherkin`, Markdown, Trymark, process,
//! snapshot, and reporter types, so a non-Gherkin frontend can keep its own
//! source paths and line numbers all the way into the outcome.

#[cfg(test)]
mod tests;
