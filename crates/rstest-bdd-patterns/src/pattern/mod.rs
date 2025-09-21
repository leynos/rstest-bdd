//! Step-pattern lexing and compilation helpers.

mod compiler;
mod lexer;
mod placeholder;

pub use compiler::build_regex_from_pattern;

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "tests exercise pattern compilation fallibility"
)]
mod tests {
    use super::build_regex_from_pattern;

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
}
