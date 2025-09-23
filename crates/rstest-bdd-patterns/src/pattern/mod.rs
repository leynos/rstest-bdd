//! Step-pattern lexing and compilation helpers.

mod compiler;
mod lexer;
mod placeholder;
#[cfg(test)]
pub(crate) mod test_support;

use crate::errors::PatternError;
use regex::Regex;

pub use compiler::build_regex_from_pattern;

/// Build and compile a regular expression from a step pattern.
///
/// # Errors
/// Returns [`PatternError`] when placeholder parsing fails or the generated
/// regex source cannot be compiled.
pub fn compile_regex_from_pattern(pat: &str) -> Result<Regex, PatternError> {
    let source = build_regex_from_pattern(pat)?;
    Regex::new(&source).map_err(PatternError::from)
}

#[cfg(test)]
mod tests {
    use super::{build_regex_from_pattern, compile_regex_from_pattern};
    use crate::errors::PatternError;

    #[test]
    fn compiles_literal_patterns() {
        #[expect(
            clippy::expect_used,
            reason = "tests require descriptive panic messages"
        )]
        let src = build_regex_from_pattern("Given a step").expect("pattern should compile");
        assert_eq!(src, "^Given a step$");
    }

    #[test]
    fn errors_on_unbalanced_braces() {
        #[expect(
            clippy::expect_used,
            reason = "tests assert placeholder parsing failures"
        )]
        let err = build_regex_from_pattern("broken {").expect_err("pattern should fail");
        assert!(err.to_string().contains("unbalanced braces"));
    }

    #[test]
    fn compiles_regex_from_pattern_successfully() {
        #[expect(
            clippy::expect_used,
            reason = "tests require descriptive panic messages"
        )]
        let regex = compile_regex_from_pattern("Given {value}").expect("pattern should compile");
        assert_eq!(regex.as_str(), "^Given (.+?)$");
    }

    #[test]
    fn surfaces_regex_compilation_errors() {
        let heavy_pattern = format!("prefix {}", "{value:f64}".repeat(20_000));
        #[expect(
            clippy::expect_used,
            reason = "tests assert regex compilation failures"
        )]
        let err =
            compile_regex_from_pattern(&heavy_pattern).expect_err("pattern should be too large");
        assert!(matches!(
            err,
            PatternError::Regex(regex::Error::CompiledTooBig(_))
        ));
    }
}
