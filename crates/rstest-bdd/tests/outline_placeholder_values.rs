//! Behavioural test for Examples values that look like placeholders.
//!
//! Placeholders are found in the original step content only, and each value
//! is inserted once and never rescanned, so a value such as `<second>` reaches
//! the step verbatim instead of being replaced by the `second` column's value.

use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd_macros::{scenario, when};

/// Records the substituted content each step receives, in step order.
///
/// Data-table cells go through the same substitution, but an outline step
/// with a table needs the fix for tables in outline constants, so this test
/// covers step text and docstrings only.
#[derive(Default)]
struct ContentJournal {
    texts: RefCell<Vec<String>>,
}

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn journal() -> ContentJournal { ContentJournal::default() }

#[when("the step text holds {first:string} and {second:string}")]
fn step_text_holds(journal: &ContentJournal, first: String, second: String) {
    journal.texts.borrow_mut().push(format!("{first} {second}"));
}

#[when("the docstring holds:")]
fn docstring_holds(journal: &ContentJournal, docstring: String) {
    journal.texts.borrow_mut().push(docstring.trim().to_owned());
}

#[scenario(path = "tests/features/outline_placeholder_values.feature")]
fn substituted_values_are_kept_verbatim(journal: ContentJournal, first: String, second: String) {
    let expected_text = format!("{first} {second}");
    assert_eq!(
        journal.texts.into_inner(),
        [expected_text.clone(), expected_text]
    );
}
