//! Behavioural tests for data tables inside scenario outlines.
//!
//! Outline steps for every Examples row are emitted into a `const` table, so
//! the generated data-table literals must be valid in a constant context. A
//! Background table folds into each outline row too, so both are covered.

use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, when};

/// Records each data table a step receives, in step order.
#[derive(Default)]
struct TableJournal {
    tables: RefCell<Vec<Vec<Vec<String>>>>,
}

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn journal() -> TableJournal { TableJournal::default() }

#[given("the background table:")]
fn background_table(journal: &TableJournal, datatable: Vec<Vec<String>>) {
    journal.tables.borrow_mut().push(datatable);
}

#[when("the outline table is:")]
fn outline_table(journal: &TableJournal, datatable: Vec<Vec<String>>) {
    journal.tables.borrow_mut().push(datatable);
}

fn row(cells: &[&str]) -> Vec<String> { cells.iter().map(|&cell| cell.to_owned()).collect() }

#[scenario(path = "tests/features/outline_datatable.feature")]
fn outline_tables_are_substituted(journal: TableJournal, item: String, count: String) {
    let expected = vec![
        vec![row(&["key", "value"]), row(&["shelf", "north"])],
        vec![
            row(&["item", "count"]),
            row(&[item.as_str(), count.as_str()]),
        ],
    ];
    assert_eq!(journal.tables.into_inner(), expected);
}
