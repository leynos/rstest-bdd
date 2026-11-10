//! Behavioural test for Examples rows whose cells are all empty.
//!
//! The journal records the substituted step text, and the scenario body
//! compares it with the row's own parameter, so a row running another row's
//! steps fails. A second test lists this binary's tests to prove the empty
//! row generated its own case rather than being dropped.

use std::{cell::RefCell, process::Command};

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

/// Lists this test binary and checks the outline produced one case per row.
///
/// The per-case assertion above cannot notice a dropped row, because the
/// surviving cases still pass, so the count is checked at the libtest boundary.
#[test]
fn every_row_generates_a_case() {
    let exe = std::env::current_exe().expect("test binary path");
    let output = Command::new(exe)
        .args(["--list", "--format", "terse"])
        .output()
        .expect("list test binary");
    let listing = String::from_utf8(output.stdout).expect("UTF-8 test listing");
    let mut cases: Vec<&str> = listing
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .filter(|name| name.starts_with("every_row_runs_its_own_steps::"))
        .collect();
    cases.sort_unstable();
    assert_eq!(
        cases,
        [
            "every_row_runs_its_own_steps::case_1",
            "every_row_runs_its_own_steps::case_2",
            "every_row_runs_its_own_steps::case_3",
        ]
    );
}
