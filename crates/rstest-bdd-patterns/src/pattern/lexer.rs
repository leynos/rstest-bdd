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
    let mut iter = pattern.char_indices().peekable();
    let mut tokens = Vec::new();
    let mut literal = String::new();

    while let Some((index, ch)) = iter.next() {
        match ch {
            BACKSLASH => handle_backslash(&mut iter, &mut literal),
            OPEN_BRACE => handle_open_brace(bytes, index, &mut iter, &mut literal, &mut tokens)?,
            CLOSE_BRACE => handle_close_brace(index, &mut iter, &mut literal, &mut tokens),
            other => literal.push(other),
        }
    }

    flush_literal(&mut literal, &mut tokens);
    Ok(tokens)
}

fn handle_backslash(
    iter: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    literal: &mut String,
) {
    if let Some((_, next)) = iter.next() {
        literal.push(next);
    } else {
        literal.push(BACKSLASH);
    }
}

fn handle_open_brace(
    bytes: &[u8],
    index: usize,
    iter: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    literal: &mut String,
    tokens: &mut Vec<Token>,
) -> Result<(), PatternError> {
    match iter.peek().copied().map(|(_, c)| c) {
        Some(OPEN_BRACE) => {
            iter.next();
            literal.push(OPEN_BRACE);
            Ok(())
        }
        Some(next) if is_placeholder_start(next) => {
            flush_literal(literal, tokens);
            parse_and_consume_placeholder(bytes, index, iter, tokens)
        }
        _ => {
            flush_literal(literal, tokens);
            tokens.push(Token::OpenBrace { index });
            Ok(())
        }
    }
}

fn handle_close_brace(
    index: usize,
    iter: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    literal: &mut String,
    tokens: &mut Vec<Token>,
) {
    if matches!(iter.peek().map(|&(_, c)| c), Some(CLOSE_BRACE)) {
        iter.next();
        literal.push(CLOSE_BRACE);
    } else {
        flush_literal(literal, tokens);
        tokens.push(Token::CloseBrace { index });
    }
}

fn is_placeholder_start(ch: char) -> bool {
    is_valid_placeholder_start(ch)
}

fn is_valid_placeholder_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn parse_and_consume_placeholder(
    bytes: &[u8],
    index: usize,
    iter: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    tokens: &mut Vec<Token>,
) -> Result<(), PatternError> {
    let (
        end,
        PlaceholderSpec {
            start, name, hint, ..
        },
    ) = parse_placeholder(bytes, index)?;
    tokens.push(Token::Placeholder { start, name, hint });
    advance_iterator_to_end(iter, end);
    Ok(())
}

fn advance_iterator_to_end(iter: &mut std::iter::Peekable<std::str::CharIndices<'_>>, end: usize) {
    while let Some(&(next_index, _)) = iter.peek() {
        if next_index < end {
            iter.next();
        } else {
            break;
        }
    }
}

fn flush_literal(literal: &mut String, tokens: &mut Vec<Token>) {
    if literal.is_empty() {
        return;
    }
    tokens.push(Token::Literal(std::mem::take(literal)));
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
