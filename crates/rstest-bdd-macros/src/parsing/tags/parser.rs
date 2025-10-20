use super::ast::{Expr, TagExprError};
use super::lexer::{Lexer, Token, TokenKind};

pub(super) struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Token,
}

impl<'a> Parser<'a> {
    pub(super) fn new(input: &'a str) -> Result<Self, TagExprError> {
        let mut lexer = Lexer::new(input);
        let current = lexer.next_token()?;
        Ok(Self { lexer, current })
    }

    pub(super) fn parse_expression(&mut self) -> Result<Expr, TagExprError> {
        self.parse_or()
    }

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

    fn advance(&mut self) -> Result<(), TagExprError> {
        self.current = self.lexer.next_token()?;
        Ok(())
    }

    fn parse_or(&mut self) -> Result<Expr, TagExprError> {
        let mut node = self.parse_and()?;
        loop {
            let token = self.current.clone();
            match token.kind {
                TokenKind::Or => {
                    self.advance()?;
                    self.ensure_operand("or")?;
                    let rhs = self.parse_and()?;
                    node = Expr::Or(Box::new(node), Box::new(rhs));
                }
                _ => break,
            }
        }
        Ok(node)
    }

    fn parse_and(&mut self) -> Result<Expr, TagExprError> {
        let mut node = self.parse_not()?;
        loop {
            let token = self.current.clone();
            match token.kind {
                TokenKind::And => {
                    self.advance()?;
                    self.ensure_operand("and")?;
                    let rhs = self.parse_not()?;
                    node = Expr::And(Box::new(node), Box::new(rhs));
                }
                _ => break,
            }
        }
        Ok(node)
    }

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
