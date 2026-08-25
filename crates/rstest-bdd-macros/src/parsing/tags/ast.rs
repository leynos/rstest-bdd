//! Abstract syntax tree and evaluation helpers for tag expressions.
//!
//! Tag expressions recognize tags (`@tag`), unary `not`, binary `and` and `or`,
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

use std::{borrow::Cow, collections::HashSet};

use super::parser::Parser;

/// Parsed representation of a tag expression.
#[derive(Clone, Debug)]
pub(crate) struct TagExpression {
    /// Root node of the parsed tag-expression tree.
    root: Expr,
}

/// Set of input tags used when evaluating a parsed expression.
type TagSet<'tags> = HashSet<Cow<'tags, str>>;

/// Node in the parsed tag-expression tree.
#[derive(Clone, Debug)]
pub(super) enum Expr {
    /// Predicate matching one tag.
    Tag(String),
    /// Negation of a nested expression.
    Not(Box<Self>),
    /// Conjunction of two expressions.
    And(Box<Self>, Box<Self>),
    /// Disjunction of two expressions.
    Or(Box<Self>, Box<Self>),
}

/// Parse error carrying the byte offset and reason for the failure.
#[derive(Debug)]
pub(crate) struct TagExprError {
    /// Byte offset at which parsing failed.
    offset: usize,
    /// Explanation of the parse failure.
    reason: String,
}

impl TagExprError {
    /// Construct an error from a byte offset and failure reason.
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
    /// Documents the internal `parse` item.
    pub(crate) fn parse(input: &str) -> Result<Self, TagExprError> {
        let mut parser = Parser::new(input)?;
        let root = parser.parse_expression()?;
        parser.expect_end()?;
        Ok(Self { root })
    }

    /// Provides the internal `evaluate` operation.
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
    /// Provides the internal `eval` operation.
    pub(super) fn eval(&self, tags: &TagSet<'_>) -> bool {
        match self {
            Self::Tag(tag) => tags.contains(tag.as_str()),
            Self::Not(inner) => !inner.eval(tags),
            Self::And(lhs, rhs) => lhs.eval(tags) && rhs.eval(tags),
            Self::Or(lhs, rhs) => lhs.eval(tags) || rhs.eval(tags),
        }
    }
}
