//! Abstract syntax tree and evaluation helpers for tag expressions.
//!
//! Tag expressions recognise tags (`@tag`), unary `not`, binary `and` and `or`,
//! and parentheses for grouping. The parser accepts nested combinations such as
//! `@fast and (not @wip or @nightly)` so macro invocations can describe complex
//! filters.
//!
//! Precedence follows Gherkin conventions: `not` binds tighter than `and`,
//! which in turn binds tighter than `or`. Operators associate to the left, so
//! `@a or @b and @c` is parsed as `@a or (@b and @c)` while chaining `and`
//! operations without parentheses still groups them left-to-right.
//!
//! Evaluation consumes the available tag set (retaining the leading `@`) and
//! applies short-circuit semantics to mirror the parser structure. This keeps
//! the filtering logic aligned with compile-time diagnostics while avoiding
//! needless work once the outcome is known.

use std::borrow::Cow;
use std::collections::HashSet;

use super::parser::Parser;

/// Parsed representation of a tag expression.
#[derive(Clone, Debug)]
pub(crate) struct TagExpression {
    root: Expr,
}

type TagSet<'tags> = HashSet<Cow<'tags, str>>;

#[derive(Clone, Debug)]
pub(super) enum Expr {
    Tag(String),
    Not(Box<Self>),
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
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

    pub(crate) fn evaluate<'tags, I, S>(&self, tags: I) -> bool
    where
        I: IntoIterator<Item = S>,
        S: Into<Cow<'tags, str>>,
    {
        let tag_set: TagSet<'tags> = tags.into_iter().map(Into::into).collect();
        // Collect tags into `Cow` so callers can provide owned `String`s or
        // borrowed `&str`s without allocating upfront for the common borrowed
        // case. The evaluator only clones when ownership is required.
        self.root.eval(&tag_set)
    }
}

impl Expr {
    pub(super) fn eval(&self, tags: &TagSet<'_>) -> bool {
        match self {
            Self::Tag(tag) => tags.contains(tag.as_str()),
            Self::Not(inner) => !inner.eval(tags),
            Self::And(lhs, rhs) => lhs.eval(tags) && rhs.eval(tags),
            Self::Or(lhs, rhs) => lhs.eval(tags) || rhs.eval(tags),
        }
    }
}
