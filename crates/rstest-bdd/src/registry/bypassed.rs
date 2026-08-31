//! Functions for recording bypassed steps during scenario skips.

use crate::{registry::StepScope, types::StepKeyword};

/// Identifies the scenario whose remaining steps were bypassed by a skip.
///
/// Grouping the scenario identity keeps [`record_bypassed_steps`] to two
/// parameters and lets generated code borrow the tag slice it already owns.
#[derive(Clone, Copy, Debug)]
pub struct BypassedScenario<'a> {
    /// Feature file that declared the scenario.
    pub feature_path: &'a str,
    /// Name of the scenario that requested the skip.
    pub scenario_name: &'a str,
    /// One-based line of the scenario heading.
    pub scenario_line: u32,
    /// Tags attached to the scenario.
    pub tags: &'a [String],
    /// Message supplied with the skip request, when present.
    pub reason: Option<&'a str>,
    /// Closed library vocabulary selected by the scenario.
    pub scope: StepScope,
}

impl<'a> BypassedScenario<'a> {
    /// Creates a descriptor from the scenario's feature path, name, and line.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::BypassedScenario;
    ///
    /// let scenario = BypassedScenario::new("tests/features/skip.feature", "skips", 7);
    /// assert_eq!(scenario.scenario_line, 7);
    /// ```
    #[must_use]
    pub const fn new(feature_path: &'a str, scenario_name: &'a str, scenario_line: u32) -> Self {
        Self {
            feature_path,
            scenario_name,
            scenario_line,
            tags: &[],
            reason: None,
            scope: StepScope::global(),
        }
    }

    /// Attaches the scenario's tags.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::BypassedScenario;
    ///
    /// let tags = vec![String::from("@allow_skipped")];
    /// let scenario = BypassedScenario::new("f.feature", "s", 1).with_tags(&tags);
    /// assert_eq!(scenario.tags.len(), 1);
    /// ```
    #[must_use]
    pub const fn with_tags(mut self, tags: &'a [String]) -> Self {
        self.tags = tags;
        self
    }

    /// Attaches the skip message.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::BypassedScenario;
    ///
    /// let scenario = BypassedScenario::new("f.feature", "s", 1).with_reason(Some("why"));
    /// assert_eq!(scenario.reason, Some("why"));
    /// ```
    #[must_use]
    pub const fn with_reason(mut self, reason: Option<&'a str>) -> Self {
        self.reason = reason;
        self
    }

    /// Attaches the scenario's closed step-library vocabulary.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::{BypassedScenario, StepScope};
    ///
    /// let scope = StepScope::global();
    /// let scenario = BypassedScenario::new("f.feature", "s", 1).with_scope(scope);
    /// assert_eq!(scenario.scope, scope);
    /// ```
    #[must_use]
    pub const fn with_scope(mut self, scope: StepScope) -> Self {
        self.scope = scope;
        self
    }
}

/// Record step definitions that were bypassed after a scenario requested a skip.
///
/// This is a no-op when the `diagnostics` feature is disabled so that generated
/// test code can reference this function unconditionally without breaking
/// `default-features = false` builds.
///
/// # Examples
///
/// ```
/// use rstest_bdd::{BypassedScenario, StepKeyword, record_bypassed_steps};
///
/// let scenario = BypassedScenario::new("tests/features/skip.feature", "skips", 7);
/// record_bypassed_steps(scenario, [(StepKeyword::Given, "a bypassed step")]);
/// ```
pub fn record_bypassed_steps<'a, I>(scenario: BypassedScenario<'_>, steps: I)
where
    I: IntoIterator<Item = (StepKeyword, &'a str)>,
{
    #[cfg(feature = "diagnostics")]
    {
        super::diagnostics::record_bypassed_steps_impl(scenario, steps);
    }

    #[cfg(not(feature = "diagnostics"))]
    {
        // Drop `steps` without iterating: the caller's iterator may be lazy,
        // expensive, or infinite, and this path promises to do no work.
        drop((scenario, steps));
    }
}
