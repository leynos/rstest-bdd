//! Behavioural test for data table support

use rstest_bdd::datatable::{self, DataTableError, DataTableRow, RowSpec, Rows};
use rstest_bdd_macros::{given, scenario};

#[given("the following table:")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "step mirrors runtime signature that hands ownership"
)]
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

#[given("a table then value {value}:")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "step mirrors runtime signature that hands ownership"
)]
fn table_then_value(datatable: Vec<Vec<String>>, value: String) {
    assert_eq!(
        datatable,
        vec![vec!["a".to_string()], vec!["b".to_string()]],
    );
    assert_eq!(value, "beta");
}

#[scenario(path = "tests/features/datatable_arg_order.feature")]
fn datatable_arg_order_scenario() {}

#[derive(Debug, PartialEq, Eq)]
struct UserRow {
    name: String,
    email: String,
    active: bool,
}

impl DataTableRow for UserRow {
    const REQUIRES_HEADER: bool = true;

    fn parse_row(mut row: RowSpec<'_>) -> Result<Self, DataTableError> {
        let name = row.take_column("name")?;
        let email = row.take_column("email")?;
        let active = row.parse_column_with("active", datatable::truthy_bool)?;
        Ok(Self {
            name,
            email,
            active,
        })
    }
}

#[given("the following users exist:")]
fn typed_users(#[datatable] rows: Rows<UserRow>) {
    let parsed: Vec<UserRow> = rows.into_iter().collect();
    assert_eq!(
        parsed,
        vec![
            UserRow {
                name: "Alice".to_string(),
                email: "alice@example.com".to_string(),
                active: true,
            },
            UserRow {
                name: "Bob".to_string(),
                email: "bob@example.com".to_string(),
                active: false,
            },
        ]
    );
}

#[scenario(path = "tests/features/datatable_typed.feature")]
fn datatable_typed_scenario() {}

#[given("the following invalid users exist:")]
fn typed_users_invalid(datatable: Vec<Vec<String>>) {
    let Err(err) = Rows::<UserRow>::try_from(datatable) else {
        panic!("expected parse failure");
    };
    assert_eq!(
        err.to_string(),
        "row 2, column 3 (active): unrecognised boolean value 'maybe' (expected yes/y/true/1 or no/n/false/0)"
    );
}

#[scenario(path = "tests/features/datatable_typed_errors.feature")]
fn datatable_typed_error_scenario() {}
