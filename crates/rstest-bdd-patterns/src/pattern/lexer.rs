//! Pattern lexer converting pattern strings into semantic tokens.

use std::iter::Peekable;
use std::str::CharIndices;

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

type CharIter<'pattern> = Peekable<CharIndices<'pattern>>;

struct LexerContext<'pattern> {
    iter: CharIter<'pattern>,
    literal: String,
    tokens: Vec<Token>,
}

impl<'pattern> LexerContext<'pattern> {
    fn new(pattern: &'pattern str) -> Self {
        Self {
            iter: pattern.char_indices().peekable(),
            literal: String::new(),
            tokens: Vec::new(),
        }
    }

    fn flush_literal(&mut self) {
        if self.literal.is_empty() {
            return;
        }

        self.tokens
            .push(Token::Literal(std::mem::take(&mut self.literal)));
    }

    fn advance_to(&mut self, end: usize) {
        while let Some(&(next_index, _)) = self.iter.peek() {
            if next_index < end {
                self.iter.next();
            } else {
                break;
            }
        }
    }

    fn into_tokens(self) -> Vec<Token> {
        self.tokens
    }
}

pub(crate) fn lex_pattern(pattern: &str) -> Result<Vec<Token>, PatternError> {
    let bytes = pattern.as_bytes();
    let mut context = LexerContext::new(pattern);

    while let Some((index, ch)) = context.iter.next() {
        match ch {
            BACKSLASH => handle_backslash(&mut context),
            OPEN_BRACE => handle_open_brace(bytes, index, &mut context)?,
            CLOSE_BRACE => handle_close_brace(index, &mut context),
            other => context.literal.push(other),
        }
    }

    context.flush_literal();
    Ok(context.into_tokens())
}

fn handle_backslash(context: &mut LexerContext<'_>) {
    if let Some((_, next)) = context.iter.next() {
        context.literal.push(next);
    } else {
        context.literal.push(BACKSLASH);
    }
}

fn handle_open_brace(
    bytes: &[u8],
    index: usize,
    context: &mut LexerContext<'_>,
) -> Result<(), PatternError> {
    match context.iter.peek().copied().map(|(_, c)| c) {
        Some(OPEN_BRACE) => {
            context.iter.next();
            context.literal.push(OPEN_BRACE);
            Ok(())
        }
        Some(next) if is_placeholder_start(next) => {
            context.flush_literal();
            parse_and_consume_placeholder(bytes, index, context)
        }
        _ => {
            context.flush_literal();
            context.tokens.push(Token::OpenBrace { index });
            Ok(())
        }
    }
}

fn handle_close_brace(index: usize, context: &mut LexerContext<'_>) {
    if matches!(context.iter.peek().map(|&(_, c)| c), Some(CLOSE_BRACE)) {
        context.iter.next();
        context.literal.push(CLOSE_BRACE);
    } else {
        context.flush_literal();
        context.tokens.push(Token::CloseBrace { index });
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
    context: &mut LexerContext<'_>,
) -> Result<(), PatternError> {
    let (
        end,
        PlaceholderSpec {
            start, name, hint, ..
        },
    ) = parse_placeholder(bytes, index)?;
    context
        .tokens
        .push(Token::Placeholder { start, name, hint });
    context.advance_to(end);
    Ok(())
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
