//! Recursive-descent parser for tag expressions.
//!
//! Implements the precedence where `or` has the lowest priority, `and` sits in
//! the middle, and `not` binds tightest, with parentheses providing explicit
//! grouping and tag identifiers acting as primaries. Diagnostics capture the
//! byte offset and unexpected token so macro errors highlight the offending
//! portion of the expression.

use std::marker::PhantomData;

use super::{
    ast::{Expr, TagExprError},
    lexer::{Lexer, Token, TokenKind},
};

/// Strategy for parsing left-associative binary operator chains.
struct ChainParseStrategy<'a, F, P, B> {
    /// Parse one operand from the current parser state.
    parse_operand: F,
    /// Identify tokens that continue this operator chain.
    is_operator: P,
    /// Operator name used in missing-operand diagnostics.
    operator_name: &'static str,
    /// Combine the left and right operands into an expression node.
    build: B,
    /// Tie the strategy lifetime to the parser callbacks.
    _marker: PhantomData<&'a ()>,
}

impl<'a, F, P, B> ChainParseStrategy<'a, F, P, B>
where
    F: FnMut(&mut Parser<'a>) -> Result<Expr, TagExprError>,
    P: FnMut(&TokenKind) -> bool,
    B: FnMut(Expr, Expr) -> Expr,
{
    /// Construct a strategy for one binary operator precedence level.
    fn new(parse_operand: F, is_operator: P, operator_name: &'static str, build: B) -> Self {
        Self {
            parse_operand,
            is_operator,
            operator_name,
            build,
            _marker: PhantomData,
        }
    }
}

/// Recursive-descent parser for a tag-expression token stream.
pub(super) struct Parser<'a> {
    /// Lexer supplying tokens from the source expression.
    lexer: Lexer<'a>,
    /// The token currently being parsed.
    current: Token,
}

impl<'a> Parser<'a> {
    /// Create a parser and read its first token.
    pub(super) fn new(input: &'a str) -> Result<Self, TagExprError> {
        let mut lexer = Lexer::new(input);
        let current = lexer.next_token()?;
        Ok(Self { lexer, current })
    }

    /// Parse a complete tag expression.
    pub(super) fn parse_expression(&mut self) -> Result<Expr, TagExprError> { self.parse_or() }

    /// Ensure that parsing consumed the entire input expression.
    pub(super) fn expect_end(&self) -> Result<(), TagExprError> {
        if matches!(self.current.kind, TokenKind::End) {
            Ok(())
        } else {
            Err(TagExprError::new(
                self.current.start,
                format!("unexpected token {}", self.current.describe()),
            ))
        }
    }

    /// Replace the current token with the next token from the lexer.
    fn advance(&mut self) -> Result<(), TagExprError> {
        self.current = self.lexer.next_token()?;
        Ok(())
    }

    /// Parse the lowest-precedence disjunction chain.
    fn parse_or(&mut self) -> Result<Expr, TagExprError> {
        let strategy = ChainParseStrategy::new(
            |parser: &mut Self| parser.parse_and(),
            |kind| matches!(kind, TokenKind::Or),
            "or",
            |lhs, rhs| Expr::Or(Box::new(lhs), Box::new(rhs)),
        );
        self.parse_chain(strategy)
    }

    /// Parse the conjunction precedence level.
    fn parse_and(&mut self) -> Result<Expr, TagExprError> {
        let strategy = ChainParseStrategy::new(
            |parser: &mut Self| parser.parse_not(),
            |kind| matches!(kind, TokenKind::And),
            "and",
            |lhs, rhs| Expr::And(Box::new(lhs), Box::new(rhs)),
        );
        self.parse_chain(strategy)
    }

    /// Parse a left-associative chain using the supplied strategy.
    fn parse_chain<F, P, B>(
        &mut self,
        mut strategy: ChainParseStrategy<'a, F, P, B>,
    ) -> Result<Expr, TagExprError>
    where
        F: FnMut(&mut Self) -> Result<Expr, TagExprError>,
        P: FnMut(&TokenKind) -> bool,
        B: FnMut(Expr, Expr) -> Expr,
    {
        let mut node = (strategy.parse_operand)(self)?;
        loop {
            let token = self.current.clone();
            if (strategy.is_operator)(&token.kind) {
                self.advance()?;
                self.ensure_operand(strategy.operator_name)?;
                let rhs = (strategy.parse_operand)(self)?;
                node = (strategy.build)(node, rhs);
            } else {
                break;
            }
        }
        Ok(node)
    }

    /// Parse prefix negation, which binds more tightly than binary operators.
    fn parse_not(&mut self) -> Result<Expr, TagExprError> {
        match self.current.kind {
            TokenKind::Not => {
                self.advance()?;
                let operand = self.parse_not()?;
                Ok(Expr::Not(Box::new(operand)))
            }
            _ => self.parse_primary(),
        }
    }

    /// Parse a tag, parenthesized expression, or end-of-input diagnostic.
    fn parse_primary(&mut self) -> Result<Expr, TagExprError> {
        match self.current.clone() {
            Token {
                kind: TokenKind::Tag(tag),
                ..
            } => {
                self.advance()?;
                Ok(Expr::Tag(tag))
            }
            Token {
                kind: TokenKind::LParen,
                ..
            } => {
                let span = self.current.start;
                self.advance()?;
                let expr = self.parse_expression()?;
                match self.current.kind {
                    TokenKind::RParen => {
                        self.advance()?;
                        Ok(expr)
                    }
                    _ => Err(TagExprError::new(span, "missing ')'")),
                }
            }
            Token {
                kind: TokenKind::End,
                start,
            } => Err(TagExprError::new(start, "expected tag or '('")),
            token => Err(TagExprError::new(
                token.start,
                format!("expected tag or '(' but found {}", token.describe()),
            )),
        }
    }

    /// Reject an operator chain that is not followed by an operand.
    fn ensure_operand(&self, name: &str) -> Result<(), TagExprError> {
        match self.current.kind {
            TokenKind::Or | TokenKind::And | TokenKind::RParen | TokenKind::End => {
                Err(TagExprError::new(
                    self.current.start,
                    format!("expected tag or '(' after '{name}'"),
                ))
            }
            _ => Ok(()),
        }
    }
}
