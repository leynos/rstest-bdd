//! Integration-style coverage for tag-expression parsing and evaluation.
//!
//! Exercises happy paths, operator precedence, and representative parse errors
//! so expression support stays aligned with the design documentation.

use rstest::rstest;

use super::TagExpression;

fn parse_expression(input: &str) -> TagExpression {
    match TagExpression::parse(input) {
        Ok(expression) => expression,
        Err(err) => panic!("parse expression `{input}`: {err}"),
    }
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

#[test]
fn evaluates_owned_and_borrowed_tags() {
    let expr = parse_expression("@fast and not @slow");
    let borrowed = vec!["@fast", "@focused"];
    assert!(expr.evaluate(borrowed));

    let owned = vec![String::from("@fast"), String::from("@focused")];
    assert!(expr.evaluate(owned));

    assert!(!expr.evaluate(vec![String::from("@slow")]));
}

#[rstest]
#[case("@a and", "expected tag or '(' after 'and'")]
#[case("@a && @b", "unexpected character '&'")]
#[case("", "expected tag or '('")]
fn reports_parse_errors(#[case] input: &str, #[case] expected: &str) {
    let err = parse_error_message(input);
    assert!(err.contains(expected), "unexpected error message: {err}");
}

#[rstest]
#[case::colon("@stage:draft", "@stage:draft")]
#[case::full_stop("@v1.2", "@v1.2")]
#[case::both("@release:v1.2", "@release:v1.2")]
fn parses_tags_with_colons_and_full_stops(#[case] input: &str, #[case] tag: &str) {
    let expr = parse_expression(input);
    assert!(expr.evaluate([tag]));
    assert!(!expr.evaluate(["@stage"]));
    assert!(!expr.evaluate(["@v1"]));
}

#[test]
fn punctuated_tags_combine_with_operators() {
    let expr = parse_expression("@stage:draft and not @v1.2");
    assert!(expr.evaluate(["@stage:draft"]));
    assert!(!expr.evaluate(["@stage:draft", "@v1.2"]));
}
