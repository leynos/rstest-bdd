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
        self.parse_chain(
            Self::parse_and,
            |kind| matches!(kind, TokenKind::Or),
            "or",
            |lhs, rhs| Expr::Or(Box::new(lhs), Box::new(rhs)),
        )
    }

    fn parse_and(&mut self) -> Result<Expr, TagExprError> {
        self.parse_chain(
            Self::parse_not,
            |kind| matches!(kind, TokenKind::And),
            "and",
            |lhs, rhs| Expr::And(Box::new(lhs), Box::new(rhs)),
        )
    }

    fn parse_chain<F, P, B>(
        &mut self,
        mut parse_operand: F,
        mut is_operator: P,
        operator_name: &'static str,
        mut build: B,
    ) -> Result<Expr, TagExprError>
    where
        F: FnMut(&mut Self) -> Result<Expr, TagExprError>,
        P: FnMut(&TokenKind) -> bool,
        B: FnMut(Expr, Expr) -> Expr,
    {
        let mut node = parse_operand(self)?;
        loop {
            let token = self.current.clone();
            if is_operator(&token.kind) {
                self.advance()?;
                self.ensure_operand(operator_name)?;
                let rhs = parse_operand(self)?;
                node = build(node, rhs);
            } else {
                break;
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
