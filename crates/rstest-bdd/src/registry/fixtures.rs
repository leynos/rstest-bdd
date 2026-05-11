//! Typed fixture requirement metadata for registered steps.
//!
//! Generated wrappers submit this sidecar metadata through `inventory` so the
//! execution layer can report missing fixture types without changing the
//! public `Step::fixtures` compatibility field.

use inventory::iter;

use crate::{StepKeyword, StepPattern};

use super::Step;

/// Name and Rust type requested for a fixture by a step definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixtureRequirement {
    /// Fixture name used for lookup in [`crate::StepContext`].
    pub name: &'static str,
    /// Rust type requested by the step parameter.
    pub ty: &'static str,
}

/// Typed fixture requirements associated with a registered step.
#[derive(Debug)]
#[doc(hidden)]
pub struct StepFixtureRequirements {
    /// The step keyword, e.g. `Given` or `When`.
    pub keyword: StepKeyword,
    /// Pattern text used to match a Gherkin step.
    pub pattern: &'static StepPattern,
    /// Typed fixture requirements for this step.
    pub requirements: &'static [FixtureRequirement],
}

/// Return typed fixture requirements for a step when generated metadata exists.
#[must_use]
pub fn fixture_requirements_for_step(step: &Step) -> Option<&'static [FixtureRequirement]> {
    iter::<StepFixtureRequirements>
        .into_iter()
        .find(|entry| {
            entry.keyword == step.keyword && entry.pattern.as_str() == step.pattern.as_str()
        })
        .map(|entry| entry.requirements)
}
