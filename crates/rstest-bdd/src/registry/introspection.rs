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
#[expect(
    clippy::expect_used,
    reason = "test code uses infallible expects for clarity"
)]
mod tests {
    //! Unit tests for registry introspection queries.

    use super::{all_steps, duplicate_steps, unused_steps};

    #[test]
    fn unused_steps_is_a_subset_of_the_registry() {
        let registered = all_steps().len();
        let unused = unused_steps().len();
        assert!(
            unused <= registered,
            "unused steps ({unused}) should never exceed registered steps ({registered})"
        );
    }

    #[test]
    fn duplicate_groups_contain_at_least_two_steps() {
        for group in duplicate_steps() {
            assert!(
                group.len() >= 2,
                "a duplicate group must contain at least two steps, found {}",
                group.len()
            );
        }
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn dump_registry_serializes_the_step_list() {
        let json = super::dump_registry().expect("registry serialization should succeed");
        assert!(
            json.contains("\"steps\""),
            "registry dump should contain a steps key: {json}"
        );
    }
}
