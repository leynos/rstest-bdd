//! Local representation of step keywords used during macro expansion.
//!
//! This lightweight enum mirrors the variants provided by `rstest-bdd` but
//! avoids a compile-time dependency on that crate.  It is only used internally
//! for parsing feature files and generating code.

use gherkin::StepType;

/// Keyword used to categorise a step definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum StepKeyword {
    /// Setup preconditions for a scenario.
    Given,
    /// Perform an action when testing behaviour.
    When,
    /// Assert the expected outcome of a scenario.
    Then,
    /// Additional conditions that share context with the previous step.
    And,
    /// Negative or contrasting conditions.
    But,
}

impl From<StepType> for StepKeyword {
    fn from(value: StepType) -> Self {
        match value {
            StepType::Given => Self::Given,
            StepType::When => Self::When,
            StepType::Then => Self::Then,
        }
    }
}
