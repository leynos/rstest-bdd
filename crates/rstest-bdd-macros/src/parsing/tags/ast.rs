//! Abstract syntax tree and evaluation helpers for tag expressions.
//!
//! The parser produces a tree of [`Expr`] nodes representing identifiers, unary
//! `not`, and binary `and`/`or` operators. Evaluation consumes a set of tags
//! (each retaining the leading `@`) and applies short-circuit semantics so
//! compile-time filtering can discard scenarios quickly.

use std::collections::HashSet;

use super::parser::Parser;

/// Parsed representation of a tag expression.
#[derive(Clone, Debug)]
pub(crate) struct TagExpression {
    root: Expr,
}

#[derive(Clone, Debug)]
pub(super) enum Expr {
    Tag(String),
    Not(Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
}

#[derive(Debug)]
pub(crate) struct TagExprError {
    offset: usize,
    reason: String,
}

impl TagExprError {
    pub(super) fn new(offset: usize, reason: impl Into<String>) -> Self {
        Self {
            offset,
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for TagExprError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid tag expression at byte {}: {}",
            self.offset, self.reason
        )
    }
}

impl std::error::Error for TagExprError {}

impl TagExpression {
    pub(crate) fn parse(input: &str) -> Result<Self, TagExprError> {
        let mut parser = Parser::new(input)?;
        let root = parser.parse_expression()?;
        parser.expect_end()?;
        Ok(Self { root })
    }

    pub(crate) fn evaluate<'a, I>(&self, tags: I) -> bool
    where
        I: IntoIterator<Item = &'a str>,
    {
        let set: HashSet<&'a str> = tags.into_iter().collect();
        self.root.eval(&set)
    }
}

impl Expr {
    pub(super) fn eval(&self, tags: &HashSet<&str>) -> bool {
        match self {
            Self::Tag(tag) => tags.contains(tag.as_str()),
            Self::Not(inner) => !inner.eval(tags),
            Self::And(lhs, rhs) => lhs.eval(tags) && rhs.eval(tags),
            Self::Or(lhs, rhs) => lhs.eval(tags) || rhs.eval(tags),
        }
    }
}
