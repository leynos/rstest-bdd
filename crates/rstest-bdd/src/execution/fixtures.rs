//! Fixture validation helpers for step execution.
//!
//! This module keeps missing-fixture diagnostic assembly separate from the
//! high-level step execution flow.

use std::collections::HashSet;
use std::sync::Arc;

use crate::context::{RSTEST_BDD_HARNESS_CONTEXT_FIXTURE, StepContext};
use crate::registry::fixture_requirements_for_step;
use crate::{FixtureRequirement, Step};

use super::StepExecutionRequest;
use super::error::{ExecutionError, MissingFixtureDiagnostic, MissingFixturesDetails};

/// Validate that all required fixtures are present in the context.
///
/// # Errors
///
/// Returns [`ExecutionError::MissingFixtures`] if any fixture listed in
/// `step.fixtures` is not available in `ctx`.
pub(super) fn validate_required_fixtures(
    step: &Step,
    ctx: &StepContext<'_>,
    request: &StepExecutionRequest<'_>,
) -> Result<(), ExecutionError> {
    if step.fixtures.is_empty() {
        Ok(())
    } else {
        let available: HashSet<&str> = ctx.available_fixtures().collect();
        let missing: Vec<_> = collect_missing(step.fixtures, &available)
            .into_iter()
            .copied()
            .collect();

        if missing.is_empty() {
            Ok(())
        } else {
            let requirements = fixture_requirements_for_step(step);
            let missing_requirements = missing_fixture_diagnostics(&missing, requirements);
            let suggestion = harness_suggestion(&missing).map(String::from);
            let available_list = sorted_available(ctx);

            Err(ExecutionError::MissingFixtures(Arc::new(
                MissingFixturesDetails {
                    step_pattern: step.pattern.as_str().to_string(),
                    step_location: format!("{}:{}", step.file, step.line),
                    required: step.fixtures.to_vec(),
                    missing,
                    missing_requirements,
                    available: available_list,
                    suggestion,
                    feature_path: request.feature_path.to_string(),
                    scenario_name: request.scenario_name.to_string(),
                },
            )))
        }
    }
}

fn collect_missing<'a>(
    fixtures: &'a [&'static str],
    available: &HashSet<&str>,
) -> Vec<&'a &'static str> {
    fixtures
        .iter()
        .filter(|fixture| !available.contains(*fixture))
        .collect()
}

fn sorted_available(ctx: &StepContext<'_>) -> Vec<String> {
    let mut list: Vec<_> = ctx.available_fixtures().map(String::from).collect();
    list.sort_unstable();
    list
}

fn missing_fixture_diagnostics(
    missing: &[&'static str],
    requirements: Option<&[FixtureRequirement]>,
) -> Vec<MissingFixtureDiagnostic> {
    missing
        .iter()
        .copied()
        .map(|name| {
            requirements
                .and_then(|requirements| {
                    requirements
                        .iter()
                        .copied()
                        .find(|requirement| requirement.name == name)
                })
                .unwrap_or(FixtureRequirement {
                    name,
                    ty: "<unknown>",
                })
                .into()
        })
        .collect()
}

fn harness_suggestion(missing: &[&str]) -> Option<&'static str> {
    missing
        .contains(&RSTEST_BDD_HARNESS_CONTEXT_FIXTURE)
        .then_some("select a harness-backed scenario so rstest_bdd_harness_context is inserted")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::FixtureRequirement;

    #[test]
    fn collect_missing_returns_absent_fixtures() {
        let available: HashSet<&str> = ["a", "b"].into_iter().collect();
        let fixtures: &[&'static str] = &["a", "c"];
        let missing = collect_missing(fixtures, &available);
        assert_eq!(missing, vec![&"c"]);
    }

    #[test]
    fn sorted_available_returns_sorted_fixture_names() {
        let mut ctx = StepContext::default();
        let v1 = 1u32;
        let v2 = 2u32;
        ctx.insert("zebra", &v1);
        ctx.insert("alpha", &v2);
        let list = sorted_available(&ctx);
        assert_eq!(list, vec!["alpha".to_string(), "zebra".to_string()]);
    }

    /// Diagnostics fall back to `<unknown>` when no typed requirements are registered.
    #[test]
    fn missing_fixture_diagnostics_falls_back_to_unknown_when_no_requirements() {
        assert_single_missing_diagnostic(None, "my_fixture", "my_fixture", "<unknown>");
    }

    /// Test helper: invokes `missing_fixture_diagnostics` and asserts the
    /// resulting single diagnostic matches `expected_name` and `expected_ty`.
    fn assert_single_missing_diagnostic(
        requirements: Option<&[FixtureRequirement]>,
        missing_fixture: &'static str,
        expected_name: &str,
        expected_ty: &str,
    ) {
        let missing = &[missing_fixture];
        let diagnostics = missing_fixture_diagnostics(missing, requirements);
        assert_eq!(diagnostics.len(), 1);
        let Some(diagnostic) = diagnostics.first() else {
            panic!("diagnostics should include the missing fixture");
        };
        assert_eq!(diagnostic.name, expected_name, "name mismatch");
        assert_eq!(diagnostic.ty, expected_ty, "type mismatch");
    }

    /// Diagnostics fall back to `<unknown>` for a fixture absent from the requirement list.
    #[test]
    fn missing_fixture_diagnostics_falls_back_when_requirement_absent() {
        assert_single_missing_diagnostic(
            Some(&[FixtureRequirement {
                name: "other_fixture",
                ty: "OtherType",
            }]),
            "my_fixture",
            "my_fixture",
            "<unknown>",
        );
    }

    /// Diagnostics use the typed requirement when present.
    #[test]
    fn missing_fixture_diagnostics_uses_typed_requirement_when_present() {
        assert_single_missing_diagnostic(
            Some(&[FixtureRequirement {
                name: "db",
                ty: "DbPool",
            }]),
            "db",
            "db",
            "DbPool",
        );
    }

    /// No harness suggestion when the harness context fixture is not in the missing list.
    #[test]
    fn harness_suggestion_absent_for_non_harness_fixture() {
        let missing = &["some_other_fixture"];
        assert!(harness_suggestion(missing).is_none());
    }

    /// Harness suggestion present when `rstest_bdd_harness_context` is missing.
    #[test]
    fn harness_suggestion_present_when_harness_context_missing() {
        let missing = &[RSTEST_BDD_HARNESS_CONTEXT_FIXTURE];
        assert!(harness_suggestion(missing).is_some());
    }

    /// No harness suggestion for an empty missing list.
    #[test]
    fn harness_suggestion_absent_for_empty_missing_list() {
        let missing: &[&str] = &[];
        assert!(harness_suggestion(missing).is_none());
    }
}
