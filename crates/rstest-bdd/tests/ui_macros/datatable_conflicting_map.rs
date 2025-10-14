//! Compile-fail UI test: datatable with both `map` and `try_map` must be rejected.

use rstest_bdd::datatable::{DataTableError, Rows};
use rstest_bdd_macros::{DataTable, DataTableRow};

#[derive(Debug, DataTableRow)]
struct Example {
    #[expect(dead_code, reason = "UI test keeps Example minimal")]
    name: String,
}

fn map_rows(_rows: Rows<Example>) -> Vec<String> {
    Vec::new()
}

fn try_map_rows(_rows: Rows<Example>) -> Result<Vec<String>, DataTableError> {
    Ok(Vec::new())
}

#[derive(Debug, DataTable)]
#[datatable(row = Example, map = map_rows)]
#[datatable(try_map = try_map_rows)]
struct Conflicting(Vec<String>);

fn main() {}
