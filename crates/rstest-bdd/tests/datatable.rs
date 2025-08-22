//! Behavioural test for data table support

use rstest_bdd::{Step, StepContext, StepError, iter, step};
use rstest_bdd_macros::{given, scenario};

#[given("the following table:")]
#[expect(clippy::needless_pass_by_value, reason = "step consumes the table")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step functions must return StepError"
)]
fn check_table(datatable: Vec<Vec<String>>) -> Result<(), StepError> {
    assert_eq!(
        datatable,
        vec![
            vec!["alpha".to_string(), "beta".to_string()],
            vec!["gamma".to_string(), "delta".to_string()],
        ],
    );
    Ok(())
}

#[scenario(path = "tests/features/datatable.feature")]
fn datatable_scenario() {}

#[given("a table then value {word}:")]
#[expect(clippy::needless_pass_by_value, reason = "step consumes the table")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step functions must return StepError"
)]
fn table_then_value(datatable: Vec<Vec<String>>, value: String) -> Result<(), StepError> {
    assert_eq!(
        datatable,
        vec![vec!["a".to_string()], vec!["b".to_string()]],
    );
    assert_eq!(value, "beta");
    Ok(())
}

#[scenario(path = "tests/features/datatable_arg_order.feature")]
fn datatable_arg_order_scenario() {}

fn requires_table_wrapper(
    _ctx: &StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    table: Option<&[&[&str]]>,
) -> Result<(), StepError> {
    let _ = table.ok_or_else(|| StepError::ExecutionError {
        step: "requires_table_wrapper".into(),
        message: "requires a data table".into(),
    })?;
    Ok(())
}

step!(
    rstest_bdd::StepKeyword::Given,
    "requires table",
    requires_table_wrapper,
    &[]
);

#[test]
fn missing_datatable_returns_execution_error() {
    let step_fn = iter::<Step>
        .into_iter()
        .find(|s| s.pattern.as_str() == "requires table")
        .map_or_else(
            || panic!("step 'requires table' not found in registry"),
            |step| step.run,
        );
    let result = step_fn(&StepContext::default(), "requires table", None, None);
    let err = match result {
        Ok(()) => panic!("expected error when datatable is missing"),
        Err(e) => e,
    };
    match err {
        StepError::ExecutionError { step, message } => {
            assert_eq!(step, "requires_table_wrapper");
            assert!(message.contains("requires a data table"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
