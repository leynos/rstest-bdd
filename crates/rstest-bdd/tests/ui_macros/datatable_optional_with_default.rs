//! Compile-fail fixture: optional fields cannot also specify defaults.

use rstest_bdd_macros::DataTableRow;

#[derive(Debug, DataTableRow)]
struct InvalidOptionalDefault {
    #[datatable(optional, default)]
    value: Option<String>,
}

fn main() {}
