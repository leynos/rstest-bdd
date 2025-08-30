//! Parser and evaluator for tag expressions shared by all macros.

use std::collections::HashSet;

/// Abstract syntax tree for a tag expression.
#[derive(Debug, PartialEq)]
pub(crate) enum Expr {
    Tag(String),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
}

/// Parse error with byte offset for diagnostics.
#[derive(Debug, PartialEq)]
pub(crate) struct ParseError {
    pub(crate) pos: usize,
    pub(crate) msg: String,
}

struct Parser<'a> {
    src: &'a [u8],
    idx: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            src: input.as_bytes(),
            idx: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.idx).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let ch = self.peek()?;
        self.idx += 1;
        Some(ch)
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\t' | b'\r')) {
            self.idx += 1;
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary(
            Self::parse_and,
            b"or",
            Expr::Or,
            "expected tag or '(' after 'or'",
        )
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary(
            Self::parse_not,
            b"and",
            Expr::And,
            "expected tag or '(' after 'and'",
        )
    }

    fn parse_binary<F>(
        &mut self,
        parse_lower: F,
        kw: &[u8],
        ctor: fn(Box<Expr>, Box<Expr>) -> Expr,
        err_after_kw: &str,
    ) -> Result<Expr, ParseError>
    where
        F: Fn(&mut Self) -> Result<Expr, ParseError>,
    {
        let mut left = parse_lower(self)?;
        loop {
            self.skip_ws();
            let start = self.idx;
            if self.consume_kw(kw) {
                self.skip_ws();
                if self.peek().is_none() {
                    return Err(ParseError {
                        pos: self.idx,
                        msg: err_after_kw.into(),
                    });
                }
                let right = parse_lower(self)?;
                left = ctor(Box::new(left), Box::new(right));
            } else {
                self.idx = start;
                break;
            }
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<Expr, ParseError> {
        self.skip_ws();
        let start = self.idx;
        if self.consume_kw(b"not") {
            let expr = self.parse_not()?;
            Ok(Expr::Not(Box::new(expr)))
        } else {
            self.idx = start;
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        self.skip_ws();
        match self.peek() {
            Some(b'@') => Ok(Expr::Tag(self.parse_tag()?)),
            Some(b'(') => {
                self.bump();
                self.skip_ws();
                if matches!(self.peek(), Some(b')')) {
                    return Err(ParseError {
                        pos: self.idx,
                        msg: "empty parentheses".into(),
                    });
                }
                let expr = self.parse_expr()?;
                self.skip_ws();
                if self.bump() != Some(b')') {
                    return Err(ParseError {
                        pos: self.idx,
                        msg: "expected ')'".into(),
                    });
                }
                Ok(expr)
            }
            Some(c) => Err(ParseError {
                pos: self.idx,
                msg: format!("unknown token '{}'", c as char),
            }),
            None => Err(ParseError {
                pos: self.idx,
                msg: "unexpected end of input".into(),
            }),
        }
    }

    fn parse_tag(&mut self) -> Result<String, ParseError> {
        self.bump(); // consume '@'
        let start = self.idx;
        let first = self.bump().ok_or_else(|| ParseError {
            pos: self.idx,
            msg: "missing tag".into(),
        })?;
        if !matches!(first, b'A'..=b'Z' | b'a'..=b'z' | b'_') {
            return Err(ParseError {
                pos: start,
                msg: "invalid tag identifier".into(),
            });
        }
        let mut buf = vec![first];
        while let Some(ch) = self.peek() {
            match ch {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' => {
                    buf.push(ch);
                    self.idx += 1;
                }
                _ => break,
            }
        }
        String::from_utf8(buf).map_err(|_| ParseError {
            pos: start,
            msg: "invalid utf8".into(),
        })
    }

    fn consume_kw(&mut self, kw: &[u8]) -> bool {
        let end = self.idx + kw.len();
        if end > self.src.len() {
            return false;
        }

        let Some(segment) = self.src.get(self.idx..end) else {
            return false;
        };
        if !segment.eq_ignore_ascii_case(kw) {
            return false;
        }

        if end < self.src.len() {
            if let Some(b) = self.src.get(end) {
                if matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'_') {
                    return false;
                }
            }
        }

        self.idx = end;
        true
    }
}

/// Parse a tag expression into an AST.
pub(crate) fn parse(input: &str) -> Result<Expr, ParseError> {
    if input.trim().is_empty() {
        return Err(ParseError {
            pos: 0,
            msg: "empty tag string is not allowed".into(),
        });
    }
    let mut p = Parser::new(input);
    let expr = p.parse_expr()?;
    p.skip_ws();
    if p.idx != p.src.len() {
        let c = p.peek().unwrap_or(b'?');
        return Err(ParseError {
            pos: p.idx,
            msg: format!("unknown token '{}'", c as char),
        });
    }
    Ok(expr)
}

/// Evaluate a parsed expression against a set of tags.
pub(crate) fn eval(expr: &Expr, tags: &HashSet<&str>) -> bool {
    match expr {
        Expr::Tag(t) => tags.contains(t.as_str()),
        Expr::And(l, r) => eval(l, tags) && eval(r, tags),
        Expr::Or(l, r) => eval(l, tags) || eval(r, tags),
        Expr::Not(e) => !eval(e, tags),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set<'a>(tags: &'a [&'a str]) -> HashSet<&'a str> {
        tags.iter().copied().collect()
    }

    #[test]
    fn honours_precedence_and_associativity() {
        let expr = match parse("@a and @b or @c") {
            Ok(e) => e,
            Err(e) => panic!("{e:?}"),
        };
        let tags = set(&["a", "b"]);
        assert!(eval(&expr, &tags));
        let tags = set(&["a", "c"]);
        assert!(eval(&expr, &tags));
        let tags = set(&["a"]);
        assert!(!eval(&expr, &tags));
    }

    #[test]
    fn operators_are_case_insensitive() {
        let expr = match parse("@a AnD Not @b") {
            Ok(e) => e,
            Err(e) => panic!("{e:?}"),
        };
        let tags = set(&["a"]);
        assert!(eval(&expr, &tags));
    }

    #[test]
    fn tags_are_case_sensitive() {
        let expr = match parse("@Smoke") {
            Ok(e) => e,
            Err(e) => panic!("{e:?}"),
        };
        let tags = set(&["smoke"]);
        assert!(!eval(&expr, &tags));
    }

    #[test]
    fn reports_empty_string() {
        let Err(err) = parse("") else {
            panic!("expected error");
        };
        assert_eq!(err.pos, 0);
    }

    #[test]
    fn reports_unknown_token() {
        let Err(err) = parse("@a && @b") else {
            panic!("expected error");
        };
        assert!(err.msg.contains("unknown token"));
    }

    #[test]
    fn reports_dangling_operator() {
        let Err(err) = parse("@a and") else {
            panic!("expected error");
        };
        assert!(err.msg.contains("expected tag"));
    }

    #[test]
    fn reports_empty_parentheses() {
        let Err(err) = parse("()") else {
            panic!("expected error");
        };
        assert!(err.msg.contains("empty parentheses"));
    }
}
