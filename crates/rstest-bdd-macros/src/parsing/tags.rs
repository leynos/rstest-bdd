use std::collections::HashSet;

/// Parsed representation of a tag expression.
#[derive(Clone, Debug)]
pub(crate) struct TagExpression {
    root: Expr,
}

#[derive(Clone, Debug)]
enum Expr {
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
    fn new(offset: usize, reason: impl Into<String>) -> Self {
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
    fn eval(&self, tags: &HashSet<&str>) -> bool {
        match self {
            Self::Tag(tag) => tags.contains(tag.as_str()),
            Self::Not(inner) => !inner.eval(tags),
            Self::And(lhs, rhs) => lhs.eval(tags) && rhs.eval(tags),
            Self::Or(lhs, rhs) => lhs.eval(tags) || rhs.eval(tags),
        }
    }
}

struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Token,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Result<Self, TagExprError> {
        let mut lexer = Lexer::new(input);
        let current = lexer.next_token()?;
        Ok(Self { lexer, current })
    }

    fn advance(&mut self) -> Result<(), TagExprError> {
        self.current = self.lexer.next_token()?;
        Ok(())
    }

    fn parse_expression(&mut self) -> Result<Expr, TagExprError> {
        self.parse_or()
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

    fn expect_end(&self) -> Result<(), TagExprError> {
        if matches!(self.current.kind, TokenKind::End) {
            Ok(())
        } else {
            Err(TagExprError::new(
                self.current.start,
                format!("unexpected token {}", self.current.describe()),
            ))
        }
    }
}

#[derive(Clone, Debug)]
struct Token {
    kind: TokenKind,
    start: usize,
}

impl Token {
    fn describe(&self) -> String {
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
enum TokenKind {
    Tag(String),
    And,
    Or,
    Not,
    LParen,
    RParen,
    End,
}

struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn next_token(&mut self) -> Result<Token, TagExprError> {
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

/// Extend the destination tag set with new values, preserving order and
/// removing duplicates.
pub(crate) fn extend_tag_set(target: &mut Vec<String>, additions: &[String]) {
    for tag in additions {
        let formatted = if tag.starts_with('@') {
            tag.clone()
        } else {
            format!("@{tag}")
        };
        if !target.iter().any(|existing| existing == &formatted) {
            target.push(formatted);
        }
    }
}

/// Merge two tag sets, preserving insertion order and de-duplicating values.
pub(crate) fn merge_tag_sets(base: &[String], additions: &[String]) -> Vec<String> {
    let mut merged = base.to_vec();
    extend_tag_set(&mut merged, additions);
    merged
}

#[cfg(test)]
mod tests {
    use super::TagExpression;

    #[test]
    fn evaluates_simple_tag() {
        let expr = match TagExpression::parse("@fast") {
            Ok(expr) => expr,
            Err(err) => panic!("parse tag expression: {err}"),
        };
        assert!(expr.evaluate(["@fast"].into_iter()));
        assert!(!expr.evaluate(["@slow"].into_iter()));
    }

    #[test]
    fn parses_hyphenated_tag() {
        let expr = match TagExpression::parse("@smoke-tests") {
            Ok(expr) => expr,
            Err(err) => panic!("parse tag expression: {err}"),
        };
        assert!(expr.evaluate(["@smoke-tests"].into_iter()));
    }

    #[test]
    fn parses_numeric_tag() {
        let expr = match TagExpression::parse("@123") {
            Ok(expr) => expr,
            Err(err) => panic!("parse tag expression: {err}"),
        };
        assert!(expr.evaluate(["@123"].into_iter()));
    }

    #[test]
    fn honours_operator_precedence() {
        let expr = match TagExpression::parse("@a or @b and @c") {
            Ok(expr) => expr,
            Err(err) => panic!("parse expression: {err}"),
        };
        assert!(expr.evaluate(["@a"].into_iter()));
        assert!(expr.evaluate(["@b", "@c"].into_iter()));
        assert!(!expr.evaluate(["@b"].into_iter()));
    }

    #[test]
    fn parses_nested_parentheses() {
        let expr = match TagExpression::parse("not (@a or @b)") {
            Ok(expr) => expr,
            Err(err) => panic!("parse expression: {err}"),
        };
        assert!(!expr.evaluate(["@a"].into_iter()));
        assert!(!expr.evaluate(["@b"].into_iter()));
        assert!(expr.evaluate(["@c"].into_iter()));
    }

    #[test]
    fn allows_case_insensitive_operators() {
        let expr = match TagExpression::parse("@a Or nOt @b") {
            Ok(expr) => expr,
            Err(err) => panic!("parse expression: {err}"),
        };
        assert!(expr.evaluate(["@a"].into_iter()));
        assert!(expr.evaluate(["@c"].into_iter()));
        assert!(!expr.evaluate(["@b"].into_iter()));
    }

    #[test]
    fn reports_missing_operand_after_and() {
        let err = match TagExpression::parse("@a and") {
            Ok(expr) => panic!("expected parse error, got {expr:?}"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("expected tag or '(' after 'and'"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn rejects_unexpected_characters() {
        let err = match TagExpression::parse("@a && @b") {
            Ok(expr) => panic!("expected parse error, got {expr:?}"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("unexpected character '&'"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn rejects_empty_expression() {
        let err = match TagExpression::parse("") {
            Ok(expr) => panic!("expected parse error, got {expr:?}"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("expected tag or '('"),
            "unexpected error message: {err}"
        );
    }
}
