//! Convert lexed tokens into anchored regular-expression sources.

use crate::errors::{PatternError, placeholder_error};
use crate::hint::get_type_pattern;

use super::lexer::{Token, lex_pattern};

/// Build an anchored regular expression from lexed pattern tokens.
///
/// # Errors
/// Returns [`PatternError`] when the tokens describe malformed placeholders or
/// unbalanced braces.
///
/// # Examples
/// ```ignore
/// # use rstest_bdd_patterns::build_regex_from_pattern;
/// let regex = build_regex_from_pattern("Given {item}")
///     .expect("example ensures fallible call succeeds");
/// assert_eq!(regex, r"^Given (.+?)$");
/// ```
pub fn build_regex_from_pattern(pat: &str) -> Result<String, PatternError> {
    let tokens = lex_pattern(pat)?;
    let mut regex = String::with_capacity(pat.len().saturating_mul(2) + 2);
    regex.push('^');
    let mut stray_depth = 0usize;

    for token in tokens {
        match token {
            Token::Literal(text) => regex.push_str(&regex::escape(&text)),
            Token::Placeholder { hint, .. } => {
                regex.push('(');
                regex.push_str(get_type_pattern(hint.as_deref()));
                regex.push(')');
            }
            Token::OpenBrace { .. } => {
                stray_depth = stray_depth.saturating_add(1);
                regex.push_str(&regex::escape("{"));
            }
            Token::CloseBrace { index } => {
                if stray_depth == 0 {
                    return Err(placeholder_error(
                        "unmatched closing brace '}' in step pattern",
                        index,
                        None,
                    ));
                }
                stray_depth -= 1;
                regex.push_str(&regex::escape("}"));
            }
        }
    }

    if stray_depth != 0 {
        return Err(placeholder_error(
            "unbalanced braces in step pattern",
            pat.len(),
            None,
        ));
    }

    regex.push('$');
    Ok(regex)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_regex_for_placeholder_patterns() {
        let regex = build_regex_from_pattern("I have {count:u32} cukes")
            .unwrap_or_else(|err| panic!("pattern should compile: {err}"));
        assert_eq!(regex, r"^I have (\d+) cukes$");
    }

    #[test]
    fn errors_when_closing_brace_unmatched() {
        let Err(err) = build_regex_from_pattern("broken}") else {
            panic!("should fail");
        };
        assert!(
            err.to_string()
                .contains("unmatched closing brace '}' in step pattern")
        );
    }

    #[test]
    fn errors_when_open_braces_remain() {
        let Err(err) = build_regex_from_pattern("{open") else {
            panic!("should fail");
        };
        assert!(
            err.to_string()
                .contains("missing closing '}' for placeholder")
        );
    }
}
