//! Tag-expression parsing, lexing, and tag-set utilities used by the macros.
//!
//! The submodules compose into a full pipeline: `lexer` tokenises raw
//! expressions, `parser` builds the [`TagExpression`] AST, `ast` evaluates
//! expressions against tag collections, and `sets` keeps tag lists normalised
//! so diagnostics are consistent.
//!
//! # Grammar
//!
//! ```text
//! expression  := disjunction
//! disjunction := conjunction ("or" conjunction)*
//! conjunction := negation ("and" negation)*
//! negation    := ("not")* primary
//! primary     := TAG | "(" expression ")"
//! TAG         := "@" IDENT
//! IDENT       := (ALNUM | "_" | "-")+
//! ```
//!
//! Expressions may nest arbitrarily using parentheses. For example, the input
//! `not (@fast or (@mobile and not @wip))` first negates the grouped
//! disjunction before the innermost conjunction is evaluated.
//!
//! # Operator precedence and associativity
//!
//! The operators follow the precedence `not` > `and` > `or`. Both `and` and
//! `or` associate from the left, so `@a or @b or @c` parses as
//! `(@a or @b) or @c`. Multiple `not` prefixes apply from the right. Explicit
//! parentheses override the default precedence when a different grouping is
//! required.
//!
//! # Tokens recognised by the lexer
//!
//! - Tag names written as `@identifier`, where identifiers may contain ASCII
//!   letters, digits, underscores, and hyphens.
//! - Case-insensitive keywords `not`, `and`, and `or`.
//! - Opening and closing parentheses for grouping.
//! - End-of-input markers used to report trailing tokens.
//!
//! # Evaluation semantics
//!
//! Parsed expressions evaluate against iterators of tag strings that include
//! the leading `@`. The evaluator short-circuits in precedence order: `or`
//! returns as soon as one operand matches, `and` stops when an operand fails,
//! and `not` negates its operand. This strategy ensures macros can efficiently
//! eliminate non-matching scenarios without scanning every tag repeatedly.
mod ast;
mod lexer;
mod parser;
mod sets;

pub(crate) use ast::TagExpression;
pub(crate) use sets::{extend_tag_set, merge_tag_sets};

#[cfg(test)]
mod tests;
