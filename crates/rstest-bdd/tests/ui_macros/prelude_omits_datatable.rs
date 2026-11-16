//! Compile-fail fixture: the prelude does not carry the datatable types.
//!
//! A step that captures a table names `rstest_bdd::DataTable`, and the users'
//! guide records the datatable items among those the prelude deliberately
//! leaves out. Importing one from the prelude must therefore fail to resolve.
//! That failure is what turns the guide's exclusion list into a fact about the
//! crate rather than a claim in prose: a reader who follows the list and a
//! reader who ignores it get different compiler verdicts.

use rstest_bdd::prelude::DataTable;

fn main() {}