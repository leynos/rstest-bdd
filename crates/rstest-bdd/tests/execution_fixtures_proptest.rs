//! Property-based tests for missing-fixture diagnostic invariants.

use std::sync::Arc;

use proptest::prelude::*;
use rstest_bdd::execution::{ExecutionError, StepExecutionRequest, execute_step};
use rstest_bdd::{StepContext, StepError, StepExecution, StepKeyword, step};

const PROPERTY_STEP_TEXT: &str = "property missing fixture diagnostics";

#[expect(
    clippy::unnecessary_wraps,
    reason = "step handlers must return Result<StepExecution, StepError>"
)]
fn property_step_wrapper(
    _ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    Ok(StepExecution::from_value(None))
}

step!(
    StepKeyword::Given,
    PROPERTY_STEP_TEXT,
    property_step_wrapper,
    &["prop_required_alpha", "prop_required_beta"]
);

fn missing_fixture_details_for(
    available_names: &[&'static str],
) -> Arc<rstest_bdd::MissingFixturesDetails> {
    let mut ctx = StepContext::default();
    let fixture_value = 1u32;
    for name in available_names {
        ctx.insert(name, &fixture_value);
    }

    let request = StepExecutionRequest {
        index: 0,
        keyword: StepKeyword::Given,
        text: PROPERTY_STEP_TEXT,
        docstring: None,
        table: None,
        feature_path: "features/property.feature",
        scenario_name: "Property diagnostics",
    };

    match execute_step(&request, &mut ctx) {
        Err(ExecutionError::MissingFixtures(details)) => details,
        other => panic!("expected missing fixture diagnostics, got {other:?}"),
    }
}

proptest! {
    /// Every fixture listed in `missing` appears in production `missing_requirements`.
    #[test]
    fn missing_requirements_covers_every_missing_fixture(
        names in prop::collection::vec("[a-z_][a-z0-9_]{0,15}", 0..8usize)
    ) {
        let static_names: Vec<&'static str> = names
            .iter()
            .map(|s| Box::leak(s.clone().into_boxed_str()) as &'static str)
            .collect();
        let details = missing_fixture_details_for(&static_names);

        for name in &details.missing {
            prop_assert!(
                details.missing_requirements.iter().any(|r| r.name == *name),
                "missing fixture '{name}' absent from missing_requirements"
            );
        }
    }

    /// Production validation always reports the available fixture list sorted.
    #[test]
    fn available_list_is_always_sorted(
        names in prop::collection::vec("[a-z_][a-z0-9_]{0,15}", 0..16usize)
    ) {
        let static_names: Vec<&'static str> = names
            .iter()
            .map(|s| Box::leak(s.clone().into_boxed_str()) as &'static str)
            .collect();
        let details = missing_fixture_details_for(&static_names);

        let mut expected = details.available.clone();
        expected.sort_unstable();
        prop_assert_eq!(&details.available, &expected);
    }
}
