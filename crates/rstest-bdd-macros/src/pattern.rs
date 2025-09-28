//! Compile-time pattern helpers shared across the macros crate.

#[cfg(feature = "compile-time-validation")]
use std::sync::OnceLock;

#[cfg(feature = "compile-time-validation")]
use proc_macro_error::abort;
#[cfg(feature = "compile-time-validation")]
use proc_macro2::Span;
#[cfg(feature = "compile-time-validation")]
use regex::Regex;

#[cfg(feature = "compile-time-validation")]
use rstest_bdd_patterns::{build_regex_from_pattern, extract_captured_values};

#[cfg(feature = "compile-time-validation")]
pub(crate) struct MacroPattern {
    text: &'static str,
    regex: OnceLock<Regex>,
}

#[cfg(feature = "compile-time-validation")]
fn abort_invalid_pattern(span: Span, pattern: &str, err: impl std::fmt::Display) -> ! {
    abort!(
        span,
        "rstest-bdd-macros: invalid step pattern `{}`: {}",
        pattern,
        err
    )
}

#[cfg(feature = "compile-time-validation")]
impl MacroPattern {
    pub(crate) const fn new(value: &'static str) -> Self {
        Self {
            text: value,
            regex: OnceLock::new(),
        }
    }

    pub(crate) const fn as_str(&self) -> &'static str {
        self.text
    }

    pub(crate) fn regex(&self, span: Span) -> &Regex {
        self.regex.get_or_init(|| {
            let source = build_regex_from_pattern(self.text)
                .unwrap_or_else(|err| abort_invalid_pattern(span, self.text, err));

            Regex::new(&source).unwrap_or_else(|err| abort_invalid_pattern(span, self.text, err))
        })
    }

    pub(crate) fn captures(&self, span: Span, text: &str) -> Option<Vec<String>> {
        extract_captured_values(self.regex(span), text)
    }
}

#[cfg(feature = "compile-time-validation")]
impl From<&'static str> for MacroPattern {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

#[cfg(all(test, feature = "compile-time-validation"))]
mod tests {
    use super::*;
    use proc_macro2::Span;

    #[test]
    fn compiles_pattern_once() {
        let pattern = MacroPattern::new("a literal step");
        let span = Span::call_site();
        let first = pattern.regex(span);
        let second = pattern.regex(span);
        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn captures_step_values() {
        let pattern = MacroPattern::new("I have {count:u32}");
        let span = Span::call_site();
        let Some(values) = pattern.captures(span, "I have 3") else {
            panic!("expected captures");
        };
        assert_eq!(values, vec!["3".to_string()]);
    }
}
