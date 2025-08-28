//! Tests for `StepKeyword` parsing behaviour.

use std::str::FromStr;

use rstest::rstest;
use rstest_bdd::{StepKeyword, panic_message};

#[rstest]
#[case("Given", StepKeyword::Given)]
#[case("given", StepKeyword::Given)]
#[case(" WHEN ", StepKeyword::When)]
#[case("then", StepKeyword::Then)]
#[case("and", StepKeyword::And)]
#[case("But", StepKeyword::But)]
fn parses_valid_keywords(#[case] input: &str, #[case] expected: StepKeyword) {
    match StepKeyword::from_str(input) {
        Ok(kw) => assert_eq!(kw, expected),
        Err(err) => panic!("unexpected error: {}", err.0),
    }
    assert_eq!(StepKeyword::from(input), expected);
}

#[test]
fn rejects_invalid_keyword() {
    match StepKeyword::from_str("unknown") {
        Ok(_) => panic!("expected an error"),
        Err(err) => assert_eq!(err.0, "unknown"),
    }
}

#[test]
fn panics_on_invalid_keyword() {
    if let Err(err) = std::panic::catch_unwind(|| StepKeyword::from("unknown")) {
        assert_eq!(panic_message(err.as_ref()), "invalid step keyword: unknown");
    } else {
        panic!("expected a panic");
    }
}
