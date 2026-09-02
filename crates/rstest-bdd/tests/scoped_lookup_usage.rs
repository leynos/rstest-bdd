//! Usage-tracking boundary tests for scoped registry queries and execution.

use rstest_bdd::{
    StepContext,
    StepKeyword,
    StepScope,
    StepText,
    execution::{StepExecutionRequest, execute_step},
    find_step_with_metadata_in_scope,
    unused_steps,
};
use rstest_bdd_macros::given;

const PATTERN: &str = "scoped query usage step";

#[given("scoped query usage step")]
fn scoped_query_usage_step() {}

fn is_unused() -> bool {
    unused_steps()
        .iter()
        .any(|step| step.pattern.as_str() == PATTERN)
}

#[test]
fn scoped_query_is_pure_but_execution_marks_the_step_used() {
    let step = find_step_with_metadata_in_scope(
        StepScope::global(),
        StepKeyword::Given,
        StepText::from(PATTERN),
    )
    .expect("scoped lookup should be unambiguous");

    assert!(step.is_some(), "registered step should resolve");
    assert!(is_unused(), "metadata queries must not mark a step used");

    let request = StepExecutionRequest {
        scope: StepScope::global(),
        index: 0,
        keyword: StepKeyword::Given,
        text: PATTERN,
        docstring: None,
        table: None,
        feature_path: "scoped_lookup_usage.feature",
        scenario_name: "Usage boundary",
    };
    execute_step(&request, &mut StepContext::default()).expect("step execution should succeed");

    assert!(!is_unused(), "execution must mark the resolved step used");
}
