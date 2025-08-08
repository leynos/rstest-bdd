//! Behavioural test for data table support

use rstest_bdd_macros::{given, scenario};

#[given("the following table:")]
#[expect(clippy::needless_pass_by_value, reason = "step consumes the table")]
fn check_table(datatable: Vec<Vec<String>>) {
    assert_eq!(
        datatable,
        vec![
            vec!["alpha".to_string(), "beta".to_string()],
            vec!["gamma".to_string(), "delta".to_string()],
        ],
    );
}

#[scenario(path = "tests/features/datatable.feature")]
fn datatable_scenario() {}
