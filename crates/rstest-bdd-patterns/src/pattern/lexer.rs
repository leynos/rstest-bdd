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
            '\\' => handle_backslash(&mut context),
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
        context.literal.push('\\');
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
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Case {
        name: &'static str,
        pattern: &'static str,
        expected: Vec<Token>,
    }

    fn literal(value: &str) -> Token {
        Token::Literal(value.into())
    }

    fn placeholder(start: usize, name: &str, hint: Option<&str>) -> Token {
        Token::Placeholder {
            start,
            name: name.into(),
            hint: hint.map(str::to_string),
        }
    }

    fn open(index: usize) -> Token {
        Token::OpenBrace { index }
    }

    fn close(index: usize) -> Token {
        Token::CloseBrace { index }
    }

    fn assert_tokens(case: &Case) {
        match lex_pattern(case.pattern) {
            Ok(tokens) => assert_eq!(tokens, case.expected, "{}", case.name),
            Err(err) => panic!(
                "{}: expected tokens but lexing failed with {err}",
                case.name
            ),
        }
    }

    #[test]
    fn lexes_patterns() {
        let cases = [
            Case {
                name: "tokenises_literals_and_placeholders",
                pattern: "Given {value:u32}",
                expected: vec![literal("Given "), placeholder(6, "value", Some("u32"))],
            },
            Case {
                name: "recognises_doubled_braces_as_literals",
                pattern: "{{outer}} {inner}",
                expected: vec![literal("{outer} "), placeholder(10, "inner", None)],
            },
            Case {
                name: "treats_nested_braces_as_placeholder",
                pattern: "before {outer {inner}} after",
                expected: vec![
                    literal("before "),
                    placeholder(7, "outer", None),
                    literal(" after"),
                ],
            },
            Case {
                name: "records_stray_braces",
                pattern: "{ literal }",
                expected: vec![open(0), literal(" literal "), close(10)],
            },
            Case {
                name: "errors_when_placeholder_starts_with_invalid_character",
                pattern: "{  value}",
                expected: vec![open(0), literal("  value"), close(8)],
            },
            Case {
                name: "preserves_multibyte_literal_segments",
                pattern: "Given café {value}",
                expected: vec![literal("Given café "), placeholder(12, "value", None)],
            },
        ];

        for case in &cases {
            assert_tokens(case);
        }
    }
}
