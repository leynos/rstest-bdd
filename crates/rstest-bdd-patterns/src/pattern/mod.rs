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

    fn build_source(pattern: &str) -> String {
        match build_regex_from_pattern(pattern) {
            Ok(source) => source,
            Err(err) => panic!("expected {pattern:?} to compile: {err}"),
        }
    }

    fn expect_build_error(pattern: &str) -> PatternError {
        match build_regex_from_pattern(pattern) {
            Ok(source) => panic!("expected {pattern:?} to fail, built {source:?}"),
            Err(err) => err,
        }
    }

    fn compile_regex_or_panic(pattern: &str) -> regex::Regex {
        match compile_regex_from_pattern(pattern) {
            Ok(regex) => regex,
            Err(err) => panic!("expected {pattern:?} to compile: {err}"),
        }
    }

    fn expect_compile_error(pattern: &str) -> PatternError {
        match compile_regex_from_pattern(pattern) {
            Ok(regex) => panic!(
                "expected {pattern:?} to fail but compiled {}",
                regex.as_str()
            ),
            Err(err) => err,
        }
    }

    #[test]
    fn compiles_literal_patterns() {
        let source = build_source("Given a step");
        assert_eq!(source, "^Given a step$");
    }

    #[test]
    fn errors_on_unbalanced_braces() {
        let err = expect_build_error("broken {");
        assert!(err.to_string().contains("unbalanced braces"));
    }

    #[test]
    fn compiles_regex_from_pattern_successfully() {
        let regex = compile_regex_or_panic("Given {value}");
        assert_eq!(regex.as_str(), "^Given (.+?)$");
    }

    #[test]
    fn surfaces_regex_compilation_errors() {
        let heavy_pattern = format!("prefix {}", "{value:f64}".repeat(20_000));
        let err = expect_compile_error(&heavy_pattern);
        assert!(matches!(
            err,
            PatternError::Regex(regex::Error::CompiledTooBig(_))
        ));
    }
}
