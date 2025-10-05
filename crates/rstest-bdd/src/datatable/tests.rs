use std::error::Error as StdError;
use std::fmt;

use super::{DataTableError, DataTableRow, RowSpec, Rows, trimmed, truthy_bool};

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

#[test]
fn parses_rows_without_header() {
    let rows = vec![
        vec!["alice".to_string(), "1".to_string()],
        vec!["bob".to_string(), "2".to_string()],
    ];
    let parsed: Rows<Pair> = rows
        .try_into()
        .unwrap_or_else(|err| panic!("table should parse: {err}"));
    assert_eq!(parsed.as_slice().len(), 2);
    let data = parsed.into_iter().collect::<Vec<_>>();
    assert_eq!(
        data,
        vec![
            Pair {
                first: "alice".to_string(),
                second: 1,
            },
            Pair {
                first: "bob".to_string(),
                second: 2,
            },
        ]
    );
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
fn parses_rows_with_header() {
    let rows = vec![
        vec!["name".to_string(), "active".to_string()],
        vec!["Alice".to_string(), "yes".to_string()],
        vec!["Bob".to_string(), "no".to_string()],
    ];
    let parsed: Rows<Named> = rows
        .try_into()
        .unwrap_or_else(|err| panic!("table should parse: {err}"));
    assert_eq!(parsed.len(), 2);
    assert_eq!(
        parsed.into_iter().collect::<Vec<_>>(),
        vec![
            Named {
                name: "Alice".to_string(),
                active: true,
            },
            Named {
                name: "Bob".to_string(),
                active: false,
            },
        ]
    );
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

#[test]
fn trimmed_preserves_inner_error() {
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct FakeError;

    impl fmt::Display for FakeError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("boom")
        }
    }

    impl StdError for FakeError {}

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
    assert_eq!(err.to_string(), "failed to parse trimmed value: boom");
}
