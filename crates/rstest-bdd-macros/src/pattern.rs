use std::fmt::Write as _;
use std::sync::OnceLock;

use proc_macro_error::abort;
use proc_macro2::Span;
use regex::Regex;

use rstest_bdd_patterns::{
    PatternError, PlaceholderErrorInfo, build_regex_from_pattern, extract_captured_values,
};

pub(crate) struct Pattern {
    text: &'static str,
    regex: OnceLock<Regex>,
}

impl Pattern {
    pub(crate) const fn new(value: &'static str) -> Self {
        Self {
            text: value,
            regex: OnceLock::new(),
        }
    }

    pub(crate) fn ensure_compiled(&self, span: Span) {
        if self.regex.get().is_some() {
            return;
        }
        let src = match build_regex_from_pattern(self.text) {
            Ok(src) => src,
            Err(err) => abort_with_error(span, self.text, err),
        };
        let regex = match Regex::new(&src) {
            Ok(regex) => regex,
            Err(err) => abort_with_error(span, self.text, PatternError::Regex(err)),
        };
        let _ = self.regex.set(regex);
    }

    pub(crate) const fn as_str(&self) -> &'static str {
        self.text
    }

    pub(crate) fn regex(&self) -> &Regex {
        self.regex
            .get()
            .unwrap_or_else(|| panic!("pattern must be compiled before matching"))
    }

    pub(crate) fn captures(&self, text: &str) -> Option<Vec<String>> {
        extract_captured_values(self.regex(), text)
    }
}

impl From<&'static str> for Pattern {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

fn format_placeholder_error(info: &PlaceholderErrorInfo) -> String {
    let mut msg = format!("{} at byte {} (zero-based)", info.message, info.position);
    if let Some(name) = &info.placeholder {
        let _ = write!(msg, " for placeholder `{name}`");
    }
    format!("invalid placeholder syntax: {msg}")
}

fn abort_with_error(span: Span, pattern: &str, err: PatternError) -> ! {
    match err {
        PatternError::Placeholder(info) => abort!(
            span,
            "rstest-bdd-macros: Invalid step pattern '{}' in #[step] macro: {}",
            pattern,
            format_placeholder_error(&info)
        ),
        PatternError::Regex(error) => abort!(
            span,
            "rstest-bdd-macros: Invalid step pattern '{}' in #[step] macro: {}",
            pattern,
            error
        ),
    }
}
