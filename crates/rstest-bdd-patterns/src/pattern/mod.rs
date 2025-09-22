//! Step-pattern lexing and compilation helpers.

mod compiler;
mod lexer;
mod placeholder;

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
#[expect(
    clippy::unwrap_used,
    reason = "tests exercise pattern compilation fallibility"
)]
mod tests {
    use super::{build_regex_from_pattern, compile_regex_from_pattern};
    use crate::errors::PatternError;

    #[test]
    fn compiles_literal_patterns() {
        let regex = build_regex_from_pattern("Given a step").unwrap();
        assert_eq!(regex, "^Given a step$");
    }

    #[test]
    fn errors_on_unbalanced_braces() {
        let err = build_regex_from_pattern("broken {").unwrap_err();
        assert!(err.to_string().contains("unbalanced braces"));
    }

    #[test]
    fn compiles_regex_from_pattern_successfully() {
        let regex = compile_regex_from_pattern("Given {value}").unwrap();
        assert_eq!(regex.as_str(), "^Given (.+?)$");
    }

    #[test]
    fn surfaces_regex_compilation_errors() {
        let heavy_pattern = format!("prefix {}", "{value:f64}".repeat(20_000));
        let err = compile_regex_from_pattern(&heavy_pattern).unwrap_err();
        assert!(matches!(
            err,
            PatternError::Regex(regex::Error::CompiledTooBig(_))
        ));
    }
}
