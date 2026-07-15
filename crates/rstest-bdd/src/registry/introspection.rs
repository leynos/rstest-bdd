//! Registry introspection helpers for diagnostic tooling.
//!
//! Exposes queries over the global step registry — unused steps, duplicate
//! definitions, and a JSON dump — consumed by `cargo bdd` and test-suite
//! health checks.

use super::{Step, StepKey, USED_STEPS, all_steps};
use hashbrown::HashMap;

/// Return registered steps that were never executed.
#[must_use]
pub fn unused_steps() -> Vec<&'static Step> {
    let used = USED_STEPS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    all_steps()
        .into_iter()
        .filter(|s| !used.contains(&(s.keyword, s.pattern)))
        .collect()
}

/// Group step definitions that share a keyword and pattern.
#[must_use]
pub fn duplicate_steps() -> Vec<Vec<&'static Step>> {
    let mut groups: HashMap<StepKey, Vec<&'static Step>> = HashMap::new();
    for step in all_steps() {
        groups
            .entry((step.keyword, step.pattern))
            .or_default()
            .push(step);
    }
    groups.into_values().filter(|v| v.len() > 1).collect()
}

/// Serialize the registry to a JSON array.
///
/// Each entry records the step keyword, pattern, source location, and whether
/// the step has been executed. The JSON is intended for consumption by
/// diagnostic tooling such as `cargo bdd`.
///
/// # Errors
///
/// Returns an error if serialization fails.
///
/// # Examples
///
/// ```
/// use rstest_bdd::dump_registry;
///
/// let json = dump_registry().expect("serialize registry");
/// assert!(json.contains("\"steps\""));
/// ```
#[cfg(feature = "diagnostics")]
pub fn dump_registry() -> serde_json::Result<String> {
    super::diagnostics::dump_registry()
}

#[cfg(test)]
mod tests {
    //! Unit tests for registry introspection queries.

    use crate::{StepContext, StepError, StepExecution, StepFuture, StepKeyword, step};

    use super::{all_steps, duplicate_steps, unused_steps};

    const USED_PATTERN: &str = "introspection used step";
    const UNUSED_PATTERN: &str = "introspection unused step";
    const DUPLICATE_PATTERN: &str = "introspection duplicate step";

    #[expect(
        clippy::unnecessary_wraps,
        reason = "test handler must match the StepFn signature"
    )]
    fn noop_step(
        _context: &mut StepContext<'_>,
        _text: &str,
        _docstring: Option<&str>,
        _table: Option<&[&[&str]]>,
    ) -> Result<StepExecution, StepError> {
        Ok(StepExecution::from_value(None))
    }

    fn noop_step_async<'ctx>(
        context: &'ctx mut StepContext<'_>,
        text: &'ctx str,
        docstring: Option<&'ctx str>,
        table: Option<&'ctx [&'ctx [&'ctx str]]>,
    ) -> StepFuture<'ctx> {
        Box::pin(std::future::ready(noop_step(
            context, text, docstring, table,
        )))
    }

    step!(
        StepKeyword::Given,
        USED_PATTERN,
        noop_step,
        noop_step_async,
        &[]
    );
    step!(
        StepKeyword::Given,
        UNUSED_PATTERN,
        noop_step,
        noop_step_async,
        &[]
    );
    step!(
        StepKeyword::When,
        DUPLICATE_PATTERN,
        noop_step,
        noop_step_async,
        &[]
    );
    step!(
        StepKeyword::When,
        DUPLICATE_PATTERN,
        noop_step,
        noop_step_async,
        &[]
    );

    #[test]
    #[expect(
        clippy::expect_used,
        reason = "test requires the registered introspection fixture"
    )]
    fn unused_steps_exclude_a_known_used_step() {
        let used_step = all_steps()
            .into_iter()
            .find(|step| step.pattern.as_str() == USED_PATTERN)
            .expect("registered introspection used step should be present");
        super::super::mark_used((used_step.keyword, used_step.pattern));

        let unused_patterns: Vec<_> = unused_steps()
            .into_iter()
            .map(|step| step.pattern.as_str())
            .collect();
        assert!(unused_patterns.contains(&UNUSED_PATTERN));
        assert!(!unused_patterns.contains(&USED_PATTERN));
    }

    #[test]
    fn duplicate_steps_report_the_registered_pattern() {
        let duplicate_patterns: Vec<Vec<_>> = duplicate_steps()
            .into_iter()
            .map(|group| {
                group
                    .into_iter()
                    .map(|step| step.pattern.as_str())
                    .collect()
            })
            .collect();
        assert!(
            duplicate_patterns
                .iter()
                .any(|patterns| { patterns.as_slice() == [DUPLICATE_PATTERN, DUPLICATE_PATTERN] })
        );
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    #[expect(
        clippy::expect_used,
        reason = "test validates required registry dump structure"
    )]
    fn dump_registry_serializes_step_state() -> serde_json::Result<()> {
        let used_step = all_steps()
            .into_iter()
            .find(|step| step.pattern.as_str() == USED_PATTERN)
            .expect("registered introspection used step should be present");
        super::super::mark_used((used_step.keyword, used_step.pattern));

        let json = super::dump_registry()?;
        let dump: serde_json::Value = serde_json::from_str(&json)?;
        let steps = dump
            .get("steps")
            .expect("registry dump should contain a steps field")
            .as_array()
            .expect("registry dump steps field should be an array");
        let dumped_used_step = steps
            .iter()
            .find(|step| step["pattern"] == USED_PATTERN)
            .expect("registry dump should contain the registered used step");
        assert_eq!(dumped_used_step["used"].as_bool(), Some(true));
        assert_eq!(dumped_used_step["bypassed"].as_bool(), Some(false));
        Ok(())
    }
}
