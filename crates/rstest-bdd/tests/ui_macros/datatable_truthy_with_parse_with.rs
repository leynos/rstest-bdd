//! Compile-fail fixture: `#[datatable(truthy)]` and `#[datatable(parse_with = ...)]`
//! may not be combined on the same field.

use rstest_bdd_macros::DataTableRow;

fn parse_bool(_value: &str) -> Result<bool, core::convert::Infallible> {
    Ok(true)
}

#[derive(Debug, DataTableRow)]
struct InvalidTruthParseField {
    #[datatable(truthy, parse_with = parse_bool)]
    value: bool,
}

fn main() {}
