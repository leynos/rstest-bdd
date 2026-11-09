//! Behavioural test for Examples rows whose cells are all empty.
//!
//! The journal records the substituted step text, and the scenario body
//! compares it with the row's own parameter, so a row running another row's
//! steps fails.

use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd_macros::{scenario, then, when};

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn journal() -> RefCell<Vec<String>> { RefCell::default() }

#[when("the label {label:string} is journalled")]
fn journal_label(journal: &RefCell<Vec<String>>, label: String) {
    journal.borrow_mut().push(label);
}

#[then("the journal holds the label {label:string}")]
fn journal_holds(journal: &RefCell<Vec<String>>, label: String) {
    assert_eq!(*journal.borrow(), [label]);
}

#[scenario(path = "tests/features/empty_example_rows.feature")]
fn every_row_runs_its_own_steps(journal: RefCell<Vec<String>>, #[case] label: String) {
    assert_eq!(journal.into_inner(), [label]);
}
