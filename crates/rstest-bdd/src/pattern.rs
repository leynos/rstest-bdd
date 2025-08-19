//! Step pattern handling and compilation.
//! This module defines `StepPattern`, a lightweight wrapper around a pattern
//! literal that compiles lazily to a regular expression.

use regex::Regex;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

/// Pattern text used to match a step at runtime.
#[derive(Debug)]
pub struct StepPattern {
    text: &'static str,
    pub(crate) regex: OnceLock<Regex>,
}

// Equality and hashing are by the underlying literal text. This allows
// `&'static StepPattern` to be used as a stable map key while keeping
// semantics intuitive and independent of allocation identity.
impl PartialEq for StepPattern {
    fn eq(&self, other: &Self) -> bool { self.text == other.text }
}

impl Eq for StepPattern {}

impl Hash for StepPattern {
    fn hash<H: Hasher>(&self, state: &mut H) { self.text.hash(state); }
}

impl StepPattern {
    /// Create a new pattern wrapper from a string literal.
    #[must_use]
    pub const fn new(value: &'static str) -> Self {
        Self {
            text: value,
            regex: OnceLock::new(),
        }
    }

    /// Access the underlying pattern string.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        self.text
    }

    /// Compile the pattern into a regular expression, caching the result.
    ///
    /// # Errors
    /// Returns an error if the pattern cannot be converted into a valid regex.
    pub fn compile(&self) -> Result<(), regex::Error> {
        let src = crate::placeholder::build_regex_from_pattern(self.text);
        let regex = Regex::new(&src)?;
        let _ = self.regex.set(regex);
        Ok(())
    }

    /// Return the cached regular expression or panic if not compiled.
    ///
    /// # Panics
    /// Panics if `compile()` was not called before this accessor.
    #[must_use]
    pub fn regex(&self) -> &Regex {
        self.regex
            .get()
            .unwrap_or_else(|| panic!("step pattern regex must be precompiled"))
    }

    /// Return the cached regex if available.
    pub(crate) fn try_regex(&self) -> Option<&Regex> {
        self.regex.get()
    }
}

impl From<&'static str> for StepPattern {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}
