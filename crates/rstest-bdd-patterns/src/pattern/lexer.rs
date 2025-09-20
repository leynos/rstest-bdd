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

const BACKSLASH: u8 = 0x5c;
const OPEN_BRACE: u8 = b'{';
const CLOSE_BRACE: u8 = b'}';

pub(crate) fn lex_pattern(pattern: &str) -> Result<Vec<Token>, PatternError> {
    let bytes = pattern.as_bytes();
    let mut tokens = Vec::new();
    let mut literal = String::new();
    let mut pos = 0;

    while let Some(&byte) = bytes.get(pos) {
        match byte {
            BACKSLASH => pos = consume_escape(bytes, pos, &mut literal),
            OPEN_BRACE => {
                if try_consume_double_open(bytes, &mut pos, &mut literal) {
                    continue;
                }
                flush_literal(&mut literal, &mut tokens);
                if try_consume_placeholder(bytes, &mut pos, &mut tokens)? {
                    continue;
                }
                tokens.push(Token::OpenBrace { index: pos });
                pos += 1;
            }
            CLOSE_BRACE => {
                if try_consume_double_close(bytes, &mut pos, &mut literal) {
                    continue;
                }
                flush_literal(&mut literal, &mut tokens);
                tokens.push(Token::CloseBrace { index: pos });
                pos += 1;
            }
            other => {
                literal.push(other as char);
                pos += 1;
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

fn consume_escape(bytes: &[u8], pos: usize, literal: &mut String) -> usize {
    if let Some(&next) = bytes.get(pos + 1) {
        literal.push(next as char);
        pos + 2
    } else {
        literal.push(char::from(BACKSLASH));
        pos + 1
    }
}

fn try_consume_double_open(bytes: &[u8], pos: &mut usize, literal: &mut String) -> bool {
    if bytes.get(*pos + 1) == Some(&OPEN_BRACE) {
        literal.push('{');
        *pos += 2;
        return true;
    }
    false
}

fn try_consume_placeholder(
    bytes: &[u8],
    pos: &mut usize,
    tokens: &mut Vec<Token>,
) -> Result<bool, PatternError> {
    let Some(&next) = bytes.get(*pos + 1) else {
        return Ok(false);
    };

    if (next as char).is_ascii_alphabetic() || next == b'_' {
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

fn try_consume_double_close(bytes: &[u8], pos: &mut usize, literal: &mut String) -> bool {
    if bytes.get(*pos + 1) == Some(&CLOSE_BRACE) {
        literal.push('}');
        *pos += 2;
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
}
