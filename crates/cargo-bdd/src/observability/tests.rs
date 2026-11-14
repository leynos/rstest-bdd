//! Unit tests for the opt-in tracing filter.

use std::ffi::OsString;

use super::{VARIABLE, filter_from};

/// No subscriber filter is built unless the opt-in variable is set.
#[test]
fn no_filter_is_built_without_the_opt_in_variable() {
    assert!(
        filter_from(None).is_none(),
        "the binary must stay silent unless {VARIABLE} is set"
    );
}

/// The variable's value becomes the filter's directive string.
#[test]
fn directives_are_read_from_the_opt_in_variable() {
    let filter = filter_from(Some(OsString::from("cargo_bdd=trace")))
        .expect("a set variable should build a filter");

    assert_eq!(filter.to_string(), "cargo_bdd=trace");
}

/// The opt-in variable name is part of the documented contract.
#[test]
fn the_opt_in_variable_is_the_documented_one() {
    assert_eq!(VARIABLE, "CARGO_BDD_LOG");
}
