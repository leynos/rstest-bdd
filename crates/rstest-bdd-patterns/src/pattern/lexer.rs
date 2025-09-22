//! Pattern lexer converting pattern strings into semantic tokens.

use crate::errors::PatternError;

use super::placeholder::{PlaceholderSpec, parse_placeholder};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Token {
    Literal(String),
    Placeholder {
        start: usize,
        name: String,
        hint: Option<String>,
    },
    OpenBrace {
        index: usize,
    },
    CloseBrace {
        index: usize,
    },
}

const BACKSLASH: char = 0x5c as char;
const OPEN_BRACE: char = '{';
const CLOSE_BRACE: char = '}';

pub(crate) fn lex_pattern(pattern: &str) -> Result<Vec<Token>, PatternError> {
    let bytes = pattern.as_bytes();
    let mut tokens = Vec::new();
    let mut literal = String::new();
    let mut pos = 0;

    while pos < pattern.len() {
        let Some(remaining) = pattern.get(pos..) else {
            break;
        };
        let mut chars = remaining.chars();
        let Some(ch) = chars.next() else {
            break;
        };
        let ch_len = ch.len_utf8();
        match ch {
            BACKSLASH => pos = consume_escape(pattern, pos, &mut literal),
            OPEN_BRACE => {
                if try_consume_double_open(pattern, &mut pos, &mut literal) {
                    continue;
                }
                flush_literal(&mut literal, &mut tokens);
                if try_consume_placeholder(pattern, bytes, &mut pos, &mut tokens)? {
                    continue;
                }
                tokens.push(Token::OpenBrace { index: pos });
                pos += ch_len;
            }
            CLOSE_BRACE => {
                if try_consume_double_close(pattern, &mut pos, &mut literal) {
                    continue;
                }
                flush_literal(&mut literal, &mut tokens);
                tokens.push(Token::CloseBrace { index: pos });
                pos += ch_len;
            }
            other => {
                literal.push(other);
                pos += ch_len;
            }
        }
    }

    flush_literal(&mut literal, &mut tokens);
    Ok(tokens)
}

fn flush_literal(literal: &mut String, tokens: &mut Vec<Token>) {
    if literal.is_empty() {
        return;
    }
    tokens.push(Token::Literal(std::mem::take(literal)));
}

fn consume_escape(pattern: &str, pos: usize, literal: &mut String) -> usize {
    let Some(remaining) = pattern.get(pos..) else {
        literal.push(BACKSLASH);
        return pattern.len();
    };
    let mut chars = remaining.chars();
    let _ = chars.next();
    if let Some(next) = chars.next() {
        literal.push(next);
        pos + BACKSLASH.len_utf8() + next.len_utf8()
    } else {
        literal.push(BACKSLASH);
        pos + BACKSLASH.len_utf8()
    }
}

fn try_consume_double_open(pattern: &str, pos: &mut usize, literal: &mut String) -> bool {
    let Some(remaining) = pattern.get(*pos..) else {
        return false;
    };
    let mut chars = remaining.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let Some(next) = chars.next() else {
        return false;
    };

    if next == OPEN_BRACE {
        *pos += first.len_utf8() + next.len_utf8();
        literal.push(OPEN_BRACE);
        return true;
    }
    false
}

fn try_consume_placeholder(
    pattern: &str,
    bytes: &[u8],
    pos: &mut usize,
    tokens: &mut Vec<Token>,
) -> Result<bool, PatternError> {
    let Some(remaining) = pattern.get(*pos..) else {
        return Ok(false);
    };
    let mut chars = remaining.chars();
    let _ = chars.next();
    let Some(next) = chars.next() else {
        return Ok(false);
    };

    if next.is_ascii_alphabetic() || next == '_' {
        let (
            next_pos,
            PlaceholderSpec {
                start, name, hint, ..
            },
        ) = parse_placeholder(bytes, *pos)?;
        tokens.push(Token::Placeholder { start, name, hint });
        *pos = next_pos;
        return Ok(true);
    }

    Ok(false)
}

fn try_consume_double_close(pattern: &str, pos: &mut usize, literal: &mut String) -> bool {
    let Some(remaining) = pattern.get(*pos..) else {
        return false;
    };
    let mut chars = remaining.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let Some(next) = chars.next() else {
        return false;
    };

    if next == CLOSE_BRACE {
        *pos += first.len_utf8() + next.len_utf8();
        literal.push(CLOSE_BRACE);
        return true;
    }
    false
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "tests exercise lexing fallibility")]
mod tests {
    use super::*;

    fn expect_tokens(pattern: &str, expected: &[Token]) {
        let tokens = lex_pattern(pattern).unwrap();
        assert_eq!(tokens.as_slice(), expected);
    }

    #[test]
    fn tokenises_literals_and_placeholders() {
        expect_tokens(
            "Given {value:u32}",
            &[
                Token::Literal("Given ".into()),
                Token::Placeholder {
                    start: 6,
                    name: "value".into(),
                    hint: Some("u32".into()),
                },
            ],
        );
    }

    #[test]
    fn recognises_doubled_braces_as_literals() {
        expect_tokens(
            "{{outer}} {inner}",
            &[
                Token::Literal("{outer} ".into()),
                Token::Placeholder {
                    start: 10,
                    name: "inner".into(),
                    hint: None,
                },
            ],
        );
    }

    #[test]
    fn treats_nested_braces_as_placeholder() {
        expect_tokens(
            "before {outer {inner}} after",
            &[
                Token::Literal("before ".into()),
                Token::Placeholder {
                    start: 7,
                    name: "outer".into(),
                    hint: None,
                },
                Token::Literal(" after".into()),
            ],
        );
    }

    #[test]
    fn records_stray_braces() {
        expect_tokens(
            "{ literal }",
            &[
                Token::OpenBrace { index: 0 },
                Token::Literal(" literal ".into()),
                Token::CloseBrace { index: 10 },
            ],
        );
    }

    #[test]
    fn errors_when_placeholder_starts_with_invalid_character() {
        expect_tokens(
            "{  value}",
            &[
                Token::OpenBrace { index: 0 },
                Token::Literal("  value".into()),
                Token::CloseBrace { index: 8 },
            ],
        );
    }

    #[test]
    fn preserves_multibyte_literal_segments() {
        expect_tokens(
            "Given café {value}",
            &[
                Token::Literal("Given café ".into()),
                Token::Placeholder {
                    start: 12,
                    name: "value".into(),
                    hint: None,
                },
            ],
        );
    }
}
