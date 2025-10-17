//! Compile-fail fixture: `#[datatable(optional)]` requires `Option<T>` fields.

use rstest_bdd_macros::DataTableRow;

#[derive(Debug, DataTableRow)]
struct InvalidOptionalField {
    #[datatable(optional)]
    value: String,
}

fn main() {}
