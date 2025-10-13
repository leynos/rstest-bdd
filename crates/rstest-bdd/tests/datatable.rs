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

#[derive(Debug, Clone, PartialEq, Eq, DataTableRow)]
struct TupleRow(String, u8, bool);

fn default_region() -> String {
    String::from("EMEA")
}

fn parse_age(value: &str) -> Result<u8, std::num::ParseIntError> {
    value.trim().parse()
}

#[derive(Debug, PartialEq, Eq, DataTable)]
struct DerivedRowCollection(Rows<DerivedRow>);

#[derive(Debug, PartialEq, Eq, DataTable)]
struct DerivedRowVecCollection(Vec<DerivedRow>);

#[derive(Debug, PartialEq, Eq, DataTable)]
#[datatable(row = DerivedRow, map = collect_active_names)]
struct ActiveNames(Vec<String>);

#[derive(Debug, PartialEq, Eq, DataTable)]
#[datatable(row = DerivedRow, try_map = collect_active_names_fallible)]
struct FallibleActiveNames(Vec<String>);

fn collect_active_names(rows: Rows<DerivedRow>) -> Vec<String> {
    rows.into_iter()
        .filter(|row| row.active)
        .map(|row| row.given_name)
        .collect()
}

fn collect_active_names_fallible(rows: Rows<DerivedRow>) -> Result<Vec<String>, DataTableError> {
    let mut names = Vec::new();
    for row in rows {
        if row.tagline == "error" {
            return Err(DataTableError::MissingColumn {
                row_number: 2,
                column: String::from("active"),
            });
        }
        if row.active {
            names.push(row.given_name.clone());
        }
    }
    Ok(names)
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
    #[expect(clippy::expect_used, reason = "test asserts successful parse")]
    let rows = Rows::<DerivedRow>::try_from(table).expect("rows should parse");
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
fn derive_data_table_row_missing_column_should_error() {
    let table = vec![
        vec![
            String::from("given-name"),
            String::from("active"),
            String::from("tagline"),
            String::from("age"),
        ],
        vec![
            String::from("Alice"),
            String::from("yes"),
            String::from(" unstoppable"),
            String::from("41"),
        ],
    ];
    #[expect(clippy::expect_used, reason = "test asserts error propagation")]
    let err = Rows::<DerivedRow>::try_from(table).expect_err("missing column");
    assert!(matches!(
        err,
        DataTableError::MissingColumn { ref column, .. } if column == "email address"
    ));
}

#[test]
fn derive_data_table_row_invalid_type_should_error() {
    let table = vec![
        vec![
            String::from("given-name"),
            String::from("email address"),
            String::from("active"),
            String::from("tagline"),
            String::from("age"),
        ],
        vec![
            String::from("Bob"),
            String::from("bob@example.com"),
            String::from("no"),
            String::from(" unstoppable"),
            String::from("not-a-number"),
        ],
    ];
    #[expect(clippy::expect_used, reason = "test asserts error propagation")]
    let err = Rows::<DerivedRow>::try_from(table).expect_err("invalid type");
    let message = err.to_string();
    assert!(
        message.contains("invalid digit"),
        "unexpected error message: {message}"
    );
}

#[test]
fn derive_data_table_row_truthy_parsing_failure_should_error() {
    let table = vec![
        vec![
            String::from("given-name"),
            String::from("email address"),
            String::from("active"),
            String::from("tagline"),
            String::from("age"),
        ],
        vec![
            String::from("Dana"),
            String::from("dana@example.com"),
            String::from("not-a-bool"),
            String::from(" unstoppable"),
            String::from("25"),
        ],
    ];
    #[expect(clippy::expect_used, reason = "test asserts error propagation")]
    let err = Rows::<DerivedRow>::try_from(table).expect_err("truthy failure");
    assert!(format!("{err}").contains("not-a-bool"));
}

#[test]
fn datatable_tuple_struct_support() {
    let table = vec![
        vec![
            String::from("Alice"),
            String::from("42"),
            String::from("true"),
        ],
        vec![
            String::from("Bob"),
            String::from("27"),
            String::from("false"),
        ],
    ];
    #[expect(clippy::expect_used, reason = "test asserts successful parse")]
    let rows = Rows::<TupleRow>::try_from(table).expect("tuple rows should parse");
    assert_eq!(
        rows.into_vec(),
        vec![
            TupleRow(String::from("Alice"), 42, true),
            TupleRow(String::from("Bob"), 27, false),
        ],
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
    #[expect(clippy::expect_used, reason = "test asserts successful parse")]
    let collection =
        DerivedRowCollection::try_from(table.clone()).expect("collection should parse");
    assert_eq!(collection.0.len(), 2);
    #[expect(clippy::expect_used, reason = "test asserts successful parse")]
    let DerivedRowVecCollection(vec_rows) =
        DerivedRowVecCollection::try_from(table.clone()).expect("vec should parse");
    assert_eq!(vec_rows.len(), 2);
    #[expect(clippy::expect_used, reason = "test asserts successful parse")]
    let ActiveNames(active) = ActiveNames::try_from(table).expect("hook should parse");
    assert_eq!(active, vec![String::from("Alice")]);
}

#[test]
fn derive_data_table_try_map_propagates_errors() {
    let table = vec![
        vec![
            String::from("given-name"),
            String::from("email address"),
            String::from("active"),
            String::from("tagline"),
            String::from("age"),
        ],
        vec![
            String::from("Eve"),
            String::from("eve@example.com"),
            String::from("yes"),
            String::from("error"),
            String::from("39"),
        ],
    ];
    #[expect(clippy::expect_used, reason = "test asserts error propagation")]
    let err = FallibleActiveNames::try_from(table).expect_err("try_map should propagate errors");
    assert!(matches!(
        err,
        DataTableError::MissingColumn { ref column, .. } if column == "active"
    ));
}
