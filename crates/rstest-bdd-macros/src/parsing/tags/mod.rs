//! Tag-expression parsing, lexing, and tag-set utilities used by the macros.
//!
//! The module exposes the AST for compile-time evaluation, along with helpers
//! for maintaining consistent precedence, validation, and normalisation.
mod ast;
mod lexer;
mod parser;
mod sets;

pub(crate) use ast::TagExpression;
pub(crate) use sets::{extend_tag_set, merge_tag_sets};

#[cfg(test)]
mod tests;
