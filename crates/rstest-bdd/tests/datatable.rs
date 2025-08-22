//! Behavioural test for data table support

use rstest_bdd::StepError;
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
