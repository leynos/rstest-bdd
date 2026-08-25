//! Tokenizes tag expressions into keywords, parentheses, and tag identifiers.
//!
//! The lexer accepts tags that already include the leading `@` and supports
//! alphanumeric, underscore, and hyphen characters. Keywords are case
//! insensitive so teams can write expressions like `@fast Or not @wip` without
//! surprises. The emitted [`Token`] stream feeds the recursive-descent parser.

use super::ast::TagExprError;

#[derive(Clone, Debug)]
/// A tag-expression token and its byte offset in the source expression.
pub(super) struct Token {
    /// The syntactic kind represented by this token.
    pub(super) kind: TokenKind,
    /// The token's starting byte offset in the input.
    pub(super) start: usize,
}

impl Token {
    /// Render the token for inclusion in parser diagnostics.
    pub(super) fn describe(&self) -> String {
        match &self.kind {
            TokenKind::Tag(tag) => tag.clone(),
            TokenKind::And => "'and'".to_owned(),
            TokenKind::Or => "'or'".to_owned(),
            TokenKind::Not => "'not'".to_owned(),
            TokenKind::LParen => "'('".to_owned(),
            TokenKind::RParen => "')'".to_owned(),
            TokenKind::End => "<end>".to_owned(),
        }
    }
}

#[derive(Clone, Debug)]
/// The lexical forms recognized in a tag expression.
pub(super) enum TokenKind {
    /// A tag identifier, including its leading `@`.
    Tag(String),
    /// The conjunction keyword.
    And,
    /// The disjunction keyword.
    Or,
    /// The negation keyword.
    Not,
    /// An opening parenthesis.
    LParen,
    /// A closing parenthesis.
    RParen,
    /// The end of the input.
    End,
}

/// Stateful lexer for tag-expression source text.
pub(super) struct Lexer<'a> {
    /// The source expression being tokenized.
    input: &'a str,
    /// The next unread byte offset.
    pos: usize,
}

impl<'a> Lexer<'a> {
    /// Create a lexer positioned at the start of `input`.
    pub(super) fn new(input: &'a str) -> Self { Self { input, pos: 0 } }

    /// Return the next token, or a diagnostic for malformed input.
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

    /// Advance past whitespace before the next token.
    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    /// Inspect the next input character without advancing the cursor.
    fn peek_char(&self) -> Option<char> {
        self.input.get(self.pos..).and_then(|s| s.chars().next())
    }

    /// Consume and return the next input character.
    fn bump_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    /// Lex a tag beginning at the supplied `@` byte offset.
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
            .to_owned();
        Ok(Token {
            kind: TokenKind::Tag(tag),
            start,
        })
    }

    /// Lex a case-insensitive boolean operator keyword.
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

/// Return whether a character may occur in a tag identifier.
fn is_tag_char(ch: char) -> bool { ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-') }
