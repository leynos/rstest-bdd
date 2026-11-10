//! Behavioural tests for tag filters naming tags with `:` or `.`.
//!
//! `gherkin` accepts tags such as `@stage:draft` and `@v1.2`, so a `tags`
//! filter must be able to name them. Each test records the label of the
//! scenario that ran and checks it is the one the filter selects.

use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd_macros::{scenario, when};

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn labels() -> RefCell<Vec<String>> { RefCell::default() }

#[when("the scenario labelled {label:string} runs")]
fn record_label(labels: &RefCell<Vec<String>>, label: String) { labels.borrow_mut().push(label); }

#[scenario(path = "tests/features/punctuated_tags.feature", tags = "@stage:draft")]
fn colon_tag_selects_its_scenario(labels: RefCell<Vec<String>>) {
    assert_eq!(labels.into_inner(), ["draft"]);
}

#[scenario(
    path = "tests/features/punctuated_tags.feature",
    name = "Versioned examples",
    tags = "@v1.2"
)]
fn full_stop_tag_selects_its_examples(labels: RefCell<Vec<String>>, label: String) {
    assert_eq!(label, "v1.2");
    assert_eq!(labels.into_inner(), [label]);
}
