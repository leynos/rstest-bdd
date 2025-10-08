//! Tests the datatable runtime parsing, error reporting, and helper parsers.

use std::error::Error as StdError;
use std::fmt;

use super::{DataTableError, DataTableRow, RowSpec, Rows, trimmed, truthy_bool};
use rstest::rstest;

#[derive(Debug, Clone, PartialEq, Eq)]
struct FakeError;

impl fmt::Display for FakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("boom")
    }
}

impl StdError for FakeError {}

#[derive(Debug, PartialEq, Eq)]
struct Pair {
    first: String,
    second: i32,
}

impl DataTableRow for Pair {
    fn parse_row(mut row: RowSpec<'_>) -> Result<Self, DataTableError> {
        let first = row.take_cell(0)?;
        let second = row.parse_with(1, trimmed)?;
        Ok(Self { first, second })
    }
}

fn assert_parses_rows<T>(
    rows: Vec<Vec<String>>,
    expected_len: usize,
    expected_data: &[T],
) -> Result<(), DataTableError>
where
    T: DataTableRow + fmt::Debug + PartialEq,
{
    let parsed: Rows<T> = rows.try_into()?;
    assert_eq!(parsed.len(), expected_len);
    let data = parsed.into_iter().collect::<Vec<_>>();
    assert_eq!(data.as_slice(), expected_data);
    Ok(())
}

#[test]
fn parses_rows_without_header() {
    assert_parses_rows(
        vec![
            vec!["alice".to_string(), "1".to_string()],
            vec!["bob".to_string(), "2".to_string()],
        ],
        2,
        &[
            Pair {
                first: "alice".to_string(),
                second: 1,
            },
            Pair {
                first: "bob".to_string(),
                second: 2,
            },
        ],
    )
    .unwrap_or_else(|err| panic!("table should parse: {err}"));
}

#[derive(Debug, PartialEq, Eq)]
struct Named {
    name: String,
    active: bool,
}

impl DataTableRow for Named {
    const REQUIRES_HEADER: bool = true;

    fn parse_row(mut row: RowSpec<'_>) -> Result<Self, DataTableError> {
        let name = row.take_column("name")?;
        let active = row.parse_column_with("active", truthy_bool)?;
        Ok(Self { name, active })
    }
}

#[test]
// Intentional: shared helper keeps duplication low while scenarios diverge on
// header handling semantics.
fn parses_rows_with_header() {
    assert_parses_rows(
        vec![
            vec!["name".to_string(), "active".to_string()],
            vec!["Alice".to_string(), "yes".to_string()],
            vec!["Bob".to_string(), "no".to_string()],
        ],
        2,
        &[
            Named {
                name: "Alice".to_string(),
                active: true,
            },
            Named {
                name: "Bob".to_string(),
                active: false,
            },
        ],
    )
    .unwrap_or_else(|err| panic!("table should parse: {err}"));
}

#[test]
fn header_is_required_when_flagged() {
    let rows = vec![
        vec!["Alice".to_string(), "yes".to_string()],
        vec!["Bob".to_string(), "no".to_string()],
    ];
    let Err(err) = Rows::<Named>::try_from(rows) else {
        panic!("missing header");
    };
    assert!(matches!(
        err,
        DataTableError::MissingColumn { column, .. } if column == "name"
    ));
}

#[test]
fn missing_header_error_is_raised_for_empty_tables() {
    let rows: Vec<Vec<String>> = Vec::new();
    let Err(err) = Rows::<Named>::try_from(rows) else {
        panic!("expected missing header error");
    };
    assert!(matches!(err, DataTableError::MissingHeader));
}

#[test]
fn uneven_rows_are_rejected() {
    #[derive(Debug)]
    struct HeaderOnly;

    impl DataTableRow for HeaderOnly {
        const REQUIRES_HEADER: bool = true;

        fn parse_row(_row: RowSpec<'_>) -> Result<Self, DataTableError> {
            Ok(Self)
        }
    }

    let rows = vec![
        vec!["name".to_string()],
        vec!["alice".to_string(), "extra".to_string()],
    ];
    let Err(err) = Rows::<HeaderOnly>::try_from(rows) else {
        panic!("uneven rows");
    };
    assert!(matches!(err, DataTableError::UnevenRow { .. }));
}

#[rstest]
#[case("yes", true)]
#[case("y", true)]
#[case("true", true)]
#[case("1", true)]
#[case("no", false)]
#[case("n", false)]
#[case("false", false)]
#[case("0", false)]
fn truthy_bool_accepts_common_forms(#[case] input: &str, #[case] expected: bool) {
    match truthy_bool(input) {
        Ok(actual) => assert_eq!(actual, expected),
        Err(err) => panic!("expected recognised boolean: {err}"),
    }
}

#[test]
fn truthy_bool_rejects_unknown_values() {
    let Err(err) = truthy_bool("maybe") else {
        panic!("value is ambiguous");
    };
    assert_eq!(
        err.to_string(),
        "unrecognised boolean value 'maybe' (expected yes/y/true/1 or no/n/false/0)"
    );
}

#[rstest]
#[case(" 42 ", 42)]
#[case("\t7\n", 7)]
fn trimmed_parses_trimmed_values(#[case] input: &str, #[case] expected: i32) {
    match trimmed::<i32>(input) {
        Ok(actual) => assert_eq!(actual, expected),
        Err(err) => panic!("expected integer to parse: {err}"),
    }
}

#[test]
fn trimmed_preserves_inner_error() {
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Dummy(u8);

    impl std::str::FromStr for Dummy {
        type Err = FakeError;

        fn from_str(_s: &str) -> Result<Self, Self::Err> {
            Err(FakeError)
        }
    }

    let Err(err) = trimmed::<Dummy>("1") else {
        panic!("expected failure");
    };
    assert_eq!(
        err.to_string(),
        "failed to parse trimmed value from input '1': boom"
    );
    assert_eq!(err.original_input(), "1");
}

#[test]
fn trimmed_reports_original_input_on_parse_failure() {
    let Err(err) = trimmed::<u8>(" 300 ") else {
        panic!("expected parse failure");
    };
    assert_eq!(err.original_input(), " 300 ");
    assert!(
        err.to_string()
            .starts_with("failed to parse trimmed value from input ' 300 '")
    );
}

#[test]
fn data_table_error_messages_cover_all_variants() {
    let duplicate = DataTableError::DuplicateHeader {
        column: "name".to_string(),
    };
    assert_eq!(
        duplicate.to_string(),
        "data table header contains duplicate column 'name'"
    );

    let uneven = DataTableError::UnevenRow {
        row_number: 3,
        expected: 2,
        actual: 3,
    };
    assert_eq!(
        uneven.to_string(),
        "data table row 3 has 3 cells but expected 2"
    );

    let missing_column = DataTableError::MissingColumn {
        row_number: 5,
        column: "age".to_string(),
    };
    assert_eq!(
        missing_column.to_string(),
        "data table row 5 is missing column 'age'"
    );

    let missing_cell = DataTableError::MissingCell {
        row_number: 2,
        column_index: 4,
    };
    assert_eq!(
        missing_cell.to_string(),
        "data table row 2 is missing cell 4"
    );

    let row_parse = DataTableError::RowParse {
        row_number: 4,
        source: Box::new(FakeError),
    };
    assert_eq!(row_parse.to_string(), "row 4: boom");

    let cell_parse = DataTableError::cell_parse(3, 1, Some("age".to_string()), FakeError);
    assert_eq!(cell_parse.to_string(), "row 3, column 2 (age): boom");
}
