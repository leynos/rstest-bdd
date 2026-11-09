//! Behavioural test for docstrings annotated with a media type.

use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario};

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn journal() -> RefCell<Vec<String>> { RefCell::default() }

#[given("the bare docstring is journalled")]
fn bare_docstring(journal: &RefCell<Vec<String>>, docstring: String) {
    journal.borrow_mut().push(docstring);
}

#[given("the annotated docstring is journalled")]
fn annotated_docstring(journal: &RefCell<Vec<String>>, docstring: String) {
    journal.borrow_mut().push(docstring);
}

#[given("the backtick docstring is journalled")]
fn backtick_docstring(journal: &RefCell<Vec<String>>, docstring: String) {
    journal.borrow_mut().push(docstring);
}

#[scenario(path = "tests/features/docstring_media_type.feature")]
fn annotated_docstrings_match_bare_ones(journal: RefCell<Vec<String>>) {
    let expected = "\n{\n  \"id\": 7\n}\n";
    assert_eq!(journal.into_inner(), [expected, expected, expected]);
}
