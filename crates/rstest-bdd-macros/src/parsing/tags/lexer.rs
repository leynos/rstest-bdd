//! Tokenises tag expressions into keywords, parentheses, and tag identifiers.
//!
//! The lexer accepts tags that already include the leading `@` and supports
//! alphanumeric, underscore, and hyphen characters. Keywords are case
//! insensitive so teams can write expressions like `@fast Or not @wip` without
//! surprises. The emitted [`Token`] stream feeds the recursive-descent parser.

use super::ast::TagExprError;

#[derive(Clone, Debug)]
pub(super) struct Token {
    pub(super) kind: TokenKind,
    pub(super) start: usize,
}

impl Token {
    pub(super) fn describe(&self) -> String {
        match &self.kind {
            TokenKind::Tag(tag) => tag.clone(),
            TokenKind::And => "'and'".to_string(),
            TokenKind::Or => "'or'".to_string(),
            TokenKind::Not => "'not'".to_string(),
            TokenKind::LParen => "'('".to_string(),
            TokenKind::RParen => "')'".to_string(),
            TokenKind::End => "<end>".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum TokenKind {
    Tag(String),
    And,
    Or,
    Not,
    LParen,
    RParen,
    End,
}

pub(super) struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub(super) fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    pub(super) fn next_token(&mut self) -> Result<Token, TagExprError> {
        self.skip_whitespace();
        if self.pos >= self.input.len() {
            return Ok(Token {
                kind: TokenKind::End,
                start: self.input.len(),
            });
        }

        let start = self.pos;
        let ch = self
            .bump_char()
            .ok_or_else(|| TagExprError::new(start, "unexpected end"))?;
        let token = match ch {
            '@' => self.lex_tag(start)?,
            '(' => Token {
                kind: TokenKind::LParen,
                start,
            },
            ')' => Token {
                kind: TokenKind::RParen,
                start,
            },
            c if c.is_ascii_alphabetic() => {
                // `lex_keyword` consumes the remainder of the identifier.
                self.lex_keyword(start)?
            }
            other => {
                return Err(TagExprError::new(
                    start,
                    format!("unexpected character '{other}'"),
                ));
            }
        };
        Ok(token)
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.input.get(self.pos..).and_then(|s| s.chars().next())
    }

    fn bump_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn lex_tag(&mut self, start: usize) -> Result<Token, TagExprError> {
        let Some(next) = self.peek_char() else {
            return Err(TagExprError::new(start + 1, "expected tag name after '@'"));
        };
        if !is_tag_char(next) {
            return Err(TagExprError::new(start + 1, "expected tag name after '@'"));
        }
        self.bump_char();
        while let Some(ch) = self.peek_char() {
            if is_tag_char(ch) {
                self.bump_char();
            } else {
                break;
            }
        }
        let tag = self
            .input
            .get(start..self.pos)
            .ok_or_else(|| TagExprError::new(start, "invalid tag boundaries"))?
            .to_string();
        Ok(Token {
            kind: TokenKind::Tag(tag),
            start,
        })
    }

    fn lex_keyword(&mut self, start: usize) -> Result<Token, TagExprError> {
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_alphabetic() {
                self.bump_char();
            } else {
                break;
            }
        }
        let end = self.pos;
        let keyword = self
            .input
            .get(start..end)
            .ok_or_else(|| TagExprError::new(start, "invalid keyword boundaries"))?;
        let lower = keyword.to_ascii_lowercase();
        let kind = match lower.as_str() {
            "and" => TokenKind::And,
            "or" => TokenKind::Or,
            "not" => TokenKind::Not,
            _ => {
                return Err(TagExprError::new(
                    start,
                    format!("unexpected identifier '{keyword}'"),
                ));
            }
        };
        Ok(Token { kind, start })
    }
}

fn is_tag_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-')
}
