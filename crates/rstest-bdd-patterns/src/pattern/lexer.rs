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

pub(crate) fn lex_pattern(pattern: &str) -> Result<Vec<Token>, PatternError> {
    let bytes = pattern.as_bytes();
    let mut tokens = Vec::new();
    let mut literal = String::new();
    let mut pos = 0;

    let flush_literal = |literal: &mut String, tokens: &mut Vec<Token>| {
        if !literal.is_empty() {
            tokens.push(Token::Literal(std::mem::take(literal)));
        }
    };

    while let Some(&b) = bytes.get(pos) {
        match b {
            b'\\' => {
                if let Some(&next) = bytes.get(pos + 1) {
                    literal.push(next as char);
                    pos += 2;
                } else {
                    literal.push('\\');
                    pos += 1;
                }
            }
            b'{' => {
                if bytes.get(pos + 1) == Some(&b'{') {
                    literal.push('{');
                    pos += 2;
                    continue;
                }
                if let Some(&next) = bytes.get(pos + 1) {
                    if (next as char).is_ascii_alphabetic() || next == b'_' {
                        flush_literal(&mut literal, &mut tokens);
                        let (
                            next_pos,
                            PlaceholderSpec {
                                start, name, hint, ..
                            },
                        ) = parse_placeholder(bytes, pos)?;
                        tokens.push(Token::Placeholder { start, name, hint });
                        pos = next_pos;
                        continue;
                    }
                }
                flush_literal(&mut literal, &mut tokens);
                tokens.push(Token::OpenBrace { index: pos });
                pos += 1;
            }
            b'}' => {
                if bytes.get(pos + 1) == Some(&b'}') {
                    literal.push('}');
                    pos += 2;
                    continue;
                }
                flush_literal(&mut literal, &mut tokens);
                tokens.push(Token::CloseBrace { index: pos });
                pos += 1;
            }
            _ => {
                literal.push(b as char);
                pos += 1;
            }
        }
    }

    flush_literal(&mut literal, &mut tokens);
    Ok(tokens)
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "tests exercise lexing fallibility")]
mod tests {
    use super::*;

    #[test]
    fn tokenises_literals_and_placeholders() {
        let tokens = lex_pattern("Given {value:u32}").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Literal("Given ".into()),
                Token::Placeholder {
                    start: 6,
                    name: "value".into(),
                    hint: Some("u32".into()),
                },
            ]
        );
    }

    #[test]
    fn recognises_doubled_braces_as_literals() {
        let tokens = lex_pattern("{{outer}} {inner}").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Literal("{outer} ".into()),
                Token::Placeholder {
                    start: 10,
                    name: "inner".into(),
                    hint: None,
                },
            ]
        );
    }

    #[test]
    fn treats_nested_braces_as_placeholder() {
        let tokens = lex_pattern("before {outer {inner}} after").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Literal("before ".into()),
                Token::Placeholder {
                    start: 7,
                    name: "outer".into(),
                    hint: None,
                },
                Token::Literal(" after".into()),
            ]
        );
    }

    #[test]
    fn records_stray_braces() {
        let tokens = lex_pattern("{ literal }").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::OpenBrace { index: 0 },
                Token::Literal(" literal ".into()),
                Token::CloseBrace { index: 10 },
            ]
        );
    }

    #[test]
    fn errors_when_placeholder_starts_with_invalid_character() {
        let tokens = lex_pattern("{  value}").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::OpenBrace { index: 0 },
                Token::Literal("  value".into()),
                Token::CloseBrace { index: 8 },
            ]
        );
    }
}
