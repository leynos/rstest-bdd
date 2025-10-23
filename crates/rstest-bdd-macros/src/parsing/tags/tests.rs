//! Integration-style coverage for tag-expression parsing and evaluation.
//!
//! Exercises happy paths, operator precedence, and representative parse errors
//! so expression support stays aligned with the design documentation.

use rstest::rstest;

use super::TagExpression;

fn parse_expression(input: &str) -> TagExpression {
    TagExpression::parse(input).unwrap_or_else(|err| panic!("parse expression `{input}`: {err}"))
}

fn parse_error_message(input: &str) -> String {
    match TagExpression::parse(input) {
        Ok(expr) => panic!("expected parse error for `{input}`, got {expr:?}"),
        Err(err) => err.to_string(),
    }
}

#[test]
fn evaluates_simple_tag() {
    let expr = parse_expression("@fast");
    assert!(expr.evaluate(["@fast"].into_iter()));
    assert!(!expr.evaluate(["@slow"].into_iter()));
}

#[test]
fn parses_hyphenated_tag() {
    let expr = parse_expression("@smoke-tests");
    assert!(expr.evaluate(["@smoke-tests"].into_iter()));
}

#[test]
fn parses_numeric_tag() {
    let expr = parse_expression("@123");
    assert!(expr.evaluate(["@123"].into_iter()));
}

#[test]
fn honours_operator_precedence() {
    let expr = parse_expression("@a or @b and @c");
    assert!(expr.evaluate(["@a"].into_iter()));
    assert!(expr.evaluate(["@b", "@c"].into_iter()));
    assert!(!expr.evaluate(["@b"].into_iter()));
}

#[test]
fn parses_nested_parentheses() {
    let expr = parse_expression("not (@a or @b)");
    assert!(!expr.evaluate(["@a"].into_iter()));
    assert!(!expr.evaluate(["@b"].into_iter()));
    assert!(expr.evaluate(["@c"].into_iter()));
}

#[test]
fn allows_case_insensitive_operators() {
    let expr = parse_expression("@a Or nOt @b");
    assert!(expr.evaluate(["@a"].into_iter()));
    assert!(expr.evaluate(["@c"].into_iter()));
    assert!(!expr.evaluate(["@b"].into_iter()));
}

#[rstest]
#[case("@a and", "expected tag or '(' after 'and'")]
#[case("@a && @b", "unexpected character '&'")]
#[case("", "expected tag or '('")]
fn reports_parse_errors(#[case] input: &str, #[case] expected: &str) {
    let err = parse_error_message(input);
    assert!(err.contains(expected), "unexpected error message: {err}");
}
