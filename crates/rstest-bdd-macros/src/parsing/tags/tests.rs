use super::TagExpression;

#[test]
fn evaluates_simple_tag() {
    let expr = match TagExpression::parse("@fast") {
        Ok(expr) => expr,
        Err(err) => panic!("parse tag expression: {err}"),
    };
    assert!(expr.evaluate(["@fast"].into_iter()));
    assert!(!expr.evaluate(["@slow"].into_iter()));
}

#[test]
fn parses_hyphenated_tag() {
    let expr = match TagExpression::parse("@smoke-tests") {
        Ok(expr) => expr,
        Err(err) => panic!("parse tag expression: {err}"),
    };
    assert!(expr.evaluate(["@smoke-tests"].into_iter()));
}

#[test]
fn parses_numeric_tag() {
    let expr = match TagExpression::parse("@123") {
        Ok(expr) => expr,
        Err(err) => panic!("parse tag expression: {err}"),
    };
    assert!(expr.evaluate(["@123"].into_iter()));
}

#[test]
fn honours_operator_precedence() {
    let expr = match TagExpression::parse("@a or @b and @c") {
        Ok(expr) => expr,
        Err(err) => panic!("parse expression: {err}"),
    };
    assert!(expr.evaluate(["@a"].into_iter()));
    assert!(expr.evaluate(["@b", "@c"].into_iter()));
    assert!(!expr.evaluate(["@b"].into_iter()));
}

#[test]
fn parses_nested_parentheses() {
    let expr = match TagExpression::parse("not (@a or @b)") {
        Ok(expr) => expr,
        Err(err) => panic!("parse expression: {err}"),
    };
    assert!(!expr.evaluate(["@a"].into_iter()));
    assert!(!expr.evaluate(["@b"].into_iter()));
    assert!(expr.evaluate(["@c"].into_iter()));
}

#[test]
fn allows_case_insensitive_operators() {
    let expr = match TagExpression::parse("@a Or nOt @b") {
        Ok(expr) => expr,
        Err(err) => panic!("parse expression: {err}"),
    };
    assert!(expr.evaluate(["@a"].into_iter()));
    assert!(expr.evaluate(["@c"].into_iter()));
    assert!(!expr.evaluate(["@b"].into_iter()));
}

#[test]
fn reports_missing_operand_after_and() {
    let err = match TagExpression::parse("@a and") {
        Ok(expr) => panic!("expected parse error, got {expr:?}"),
        Err(err) => err,
    };
    assert!(
        err.to_string().contains("expected tag or '(' after 'and'"),
        "unexpected error message: {err}"
    );
}

#[test]
fn rejects_unexpected_characters() {
    let err = match TagExpression::parse("@a && @b") {
        Ok(expr) => panic!("expected parse error, got {expr:?}"),
        Err(err) => err,
    };
    assert!(
        err.to_string().contains("unexpected character '&'"),
        "unexpected error message: {err}"
    );
}

#[test]
fn rejects_empty_expression() {
    let err = match TagExpression::parse("") {
        Ok(expr) => panic!("expected parse error, got {expr:?}"),
        Err(err) => err,
    };
    assert!(
        err.to_string().contains("expected tag or '('"),
        "unexpected error message: {err}"
    );
}
