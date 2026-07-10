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
///
/// # Global state
///
/// This function queries the `inventory`-based global registry populated at
/// link time. It is intentionally not dependency-injected; the registry is the
/// source of truth for the entire step-execution pipeline.
#[must_use]
pub fn fixture_requirements_for_step(step: &Step) -> Option<&'static [FixtureRequirement]> {
    iter::<StepFixtureRequirements>
        .into_iter()
        .find(|entry| {
            entry.keyword == step.keyword && entry.pattern.as_str() == step.pattern.as_str()
        })
        .map(|entry| entry.requirements)
}

#[cfg(test)]
mod tests {
    //! Unit tests for step fixture requirement metadata.

    use super::*;
    use crate::StepKeyword;
    use crate::registry::Step;

    /// A sentinel pattern used only by the unit tests below.
    static UNIT_TEST_PATTERN: crate::StepPattern =
        crate::StepPattern::new("__unit_test_fixture_requirements__");

    static UNIT_TEST_REQUIREMENTS: [FixtureRequirement; 1] = [FixtureRequirement {
        name: "unit_test_fixture",
        ty: "UnitTestType",
    }];

    // Register a sidecar entry visible within this test binary.
    inventory::submit! {
        StepFixtureRequirements {
            keyword: StepKeyword::Given,
            pattern: &UNIT_TEST_PATTERN,
            requirements: &UNIT_TEST_REQUIREMENTS,
        }
    }

    #[expect(
        clippy::unnecessary_wraps,
        reason = "StepFn-compatible test handlers must return Result"
    )]
    fn noop_step(
        _ctx: &mut crate::StepContext<'_>,
        _text: &str,
        _docstring: Option<&str>,
        _table: Option<&[&[&str]]>,
    ) -> Result<crate::StepExecution, crate::StepError> {
        Ok(crate::StepExecution::from_value(None))
    }

    fn noop_step_async<'ctx>(
        ctx: &'ctx mut crate::StepContext<'_>,
        text: &'ctx str,
        docstring: Option<&'ctx str>,
        table: Option<&'ctx [&'ctx [&'ctx str]]>,
    ) -> crate::StepFuture<'ctx> {
        Box::pin(std::future::ready(noop_step(ctx, text, docstring, table)))
    }

    /// Returns `None` when no sidecar is registered for the step.
    #[test]
    fn fixture_requirements_for_step_returns_none_when_no_sidecar() {
        static MISSING_PATTERN: crate::StepPattern =
            crate::StepPattern::new("__no_sidecar_registered__");

        // Build a minimal Step-like value; only keyword/pattern are inspected.
        let step = Step {
            keyword: StepKeyword::When,
            pattern: &MISSING_PATTERN,
            run: noop_step,
            run_async: noop_step_async,
            execution_mode: crate::StepExecutionMode::Both,
            fixtures: &[],
            file: file!(),
            line: line!(),
        };

        assert!(
            fixture_requirements_for_step(&step).is_none(),
            "expected None for a step with no registered sidecar"
        );
    }

    /// Returns `Some` with the correct requirements when a sidecar is registered.
    #[test]
    fn fixture_requirements_for_step_returns_requirements_when_sidecar_present() {
        let step = Step {
            keyword: StepKeyword::Given,
            pattern: &UNIT_TEST_PATTERN,
            run: noop_step,
            run_async: noop_step_async,
            execution_mode: crate::StepExecutionMode::Both,
            fixtures: &[],
            file: file!(),
            line: line!(),
        };

        let Some(requirements) = fixture_requirements_for_step(&step) else {
            panic!("sidecar was submitted via inventory::submit! above");
        };

        assert_eq!(requirements.len(), 1);
        let Some(requirement) = requirements.first() else {
            panic!("requirements should include the submitted fixture");
        };
        assert_eq!(requirement.name, "unit_test_fixture");
        assert_eq!(requirement.ty, "UnitTestType");
    }
}
