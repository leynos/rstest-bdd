//! Behavioural test for data table support

use rstest_bdd::datatable::{self, DataTableError, DataTableRow, RowSpec, Rows};
use rstest_bdd_macros::{DataTable, DataTableRow, given, scenario};

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

#[derive(Debug, Clone, PartialEq, Eq, DataTableRow)]
#[datatable(rename_all = "kebab-case")]
struct DerivedRow {
    given_name: String,
    #[datatable(column = "email address")]
    email: String,
    #[datatable(truthy)]
    active: bool,
    #[datatable(optional)]
    nickname: Option<String>,
    #[datatable(default = default_region)]
    region: String,
    #[datatable(trim)]
    tagline: String,
    #[datatable(parse_with = parse_age)]
    age: u8,
}

fn default_region() -> String {
    String::from("EMEA")
}

fn parse_age(value: &str) -> Result<u8, std::num::ParseIntError> {
    value.trim().parse()
}

#[derive(Debug, PartialEq, Eq, DataTable)]
struct DerivedRowCollection(Rows<DerivedRow>);

#[derive(Debug, PartialEq, Eq, DataTable)]
#[datatable(row = DerivedRow, map = collect_active_names)]
struct ActiveNames(Vec<String>);

fn collect_active_names(rows: Rows<DerivedRow>) -> Vec<String> {
    rows.into_iter()
        .filter(|row| row.active)
        .map(|row| row.given_name)
        .collect()
}

#[test]
fn derive_data_table_row_parses_and_maps_columns() {
    let table = vec![
        vec![
            String::from("given-name"),
            String::from("email address"),
            String::from("active"),
            String::from("tagline"),
            String::from("age"),
        ],
        vec![
            String::from("Alice"),
            String::from("alice@example.com"),
            String::from("yes"),
            String::from("  unstoppable  "),
            String::from(" 42 "),
        ],
    ];
    let rows = match Rows::<DerivedRow>::try_from(table) {
        Ok(rows) => rows,
        Err(err) => panic!("rows should parse: {err}"),
    };
    assert_eq!(
        rows.into_vec(),
        vec![DerivedRow {
            given_name: String::from("Alice"),
            email: String::from("alice@example.com"),
            active: true,
            nickname: None,
            region: String::from("EMEA"),
            tagline: String::from("unstoppable"),
            age: 42,
        }],
    );
}

#[test]
fn derive_data_table_supports_collection_wrappers_and_hooks() {
    let table = vec![
        vec![
            String::from("given-name"),
            String::from("email address"),
            String::from("active"),
            String::from("tagline"),
            String::from("age"),
        ],
        vec![
            String::from("Alice"),
            String::from("alice@example.com"),
            String::from("yes"),
            String::from(" unstoppable"),
            String::from("41"),
        ],
        vec![
            String::from("Bob"),
            String::from("bob@example.com"),
            String::from("no"),
            String::from("tenacious"),
            String::from("43"),
        ],
    ];
    let collection = match DerivedRowCollection::try_from(table.clone()) {
        Ok(collection) => collection,
        Err(err) => panic!("collection should parse: {err}"),
    };
    assert_eq!(collection.0.len(), 2);
    let ActiveNames(active) = match ActiveNames::try_from(table) {
        Ok(active) => active,
        Err(err) => panic!("hook should parse: {err}"),
    };
    assert_eq!(active, vec![String::from("Alice")]);
}
