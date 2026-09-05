//! Helpers for struct-based step arguments parsed from pattern placeholders.
//!
//! The [`StepArgs`] trait marks structs whose fields should be populated from
//! the textual captures produced by a step pattern. Implementations record the
//! field names for diagnostics and provide conversion logic from the ordered
//! vector of capture strings emitted by the wrapper. Deriving the trait via
//! `rstest_bdd_macros::StepArgs` enforces the required `FromStr` bounds and
//! surfaces parse failures as [`StepArgsError`] values.

use std::fmt;

/// Error returned when converting captured placeholder strings into a struct
/// annotated with `#[derive(StepArgs)]` fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepArgsError {
    /// Human-readable conversion failure message.
    message: String,
}

/// One named placeholder capture supplied to a [`StepArgs`] implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepCapture {
    /// Placeholder name from the step pattern.
    pub name: &'static str,
    /// Capture text after pattern-level normalization.
    pub value: String,
}

impl StepArgsError {
    /// Construct a new error with the provided message.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Build an error describing a failed parse for `field` using `raw`.
    #[must_use]
    pub fn parse_failure(field: &'static str, raw: &str) -> Self {
        Self::new(format!(
            "failed to parse field '{field}' from value '{raw}'"
        ))
    }

    /// Build an error preserving a custom scalar parser's diagnostic text.
    #[must_use]
    pub fn custom_parse_failure(
        field: &'static str,
        raw: &str,
        error: impl std::fmt::Display,
    ) -> Self {
        Self::new(format!(
            "failed to parse field '{field}' from value '{raw}': {error}"
        ))
    }

    /// Build an error describing a mismatch between expected and actual counts.
    ///
    /// The derive macro validates capture counts at compile time, but the
    /// constructor remains available for manual implementations.
    #[must_use]
    pub fn count_mismatch(expected: usize, actual: usize) -> Self {
        Self::new(format!(
            "expected {expected} captured value(s) but received {actual}"
        ))
    }

    /// Build an error describing a missing named placeholder.
    #[must_use]
    pub fn missing_field(field: &'static str, placeholder: &'static str) -> Self {
        Self::new(format!(
            "field '{field}' requires placeholder '{{{placeholder}}}'"
        ))
    }

    /// Build an error describing a placeholder not claimed by the aggregate.
    #[must_use]
    pub fn unconsumed_capture(placeholder: &str) -> Self {
        Self::new(format!("unconsumed placeholder '{{{placeholder}}}'"))
    }

    /// Build an error describing a duplicate placeholder capture.
    #[must_use]
    pub fn duplicate_capture(placeholder: &str) -> Self {
        Self::new(format!("duplicate placeholder capture '{{{placeholder}}}'"))
    }

    /// Access the underlying error message.
    #[must_use]
    pub fn message(&self) -> &str { &self.message }
}

impl fmt::Display for StepArgsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.message) }
}

impl std::error::Error for StepArgsError {}

/// Trait implemented by structs populated from placeholder captures.
pub trait StepArgs: Sized {
    /// Number of fields participating in the capture mapping.
    const FIELD_COUNT: usize;
    /// Field names in declaration order. Used for documentation and future
    /// diagnostics.
    const FIELD_NAMES: &'static [&'static str];

    /// Convert the ordered capture strings into a populated struct.
    ///
    /// # Errors
    /// Returns [`StepArgsError`] when the conversion fails (for example when a
    /// field cannot be parsed into the requested type).
    fn from_captures(values: Vec<String>) -> Result<Self, StepArgsError>;

    /// Convert named captures into a populated struct.
    ///
    /// The default validates the named record and passes values to a manual
    /// [`Self::from_captures`] implementation in [`Self::FIELD_NAMES`] order.
    /// Derived implementations override it to parse fields directly by name.
    ///
    /// # Errors
    /// Returns [`StepArgsError`] when the captures cannot populate the value.
    fn from_named_captures(captures: Vec<StepCapture>) -> Result<Self, StepArgsError> {
        if captures.len() != Self::FIELD_COUNT {
            return Err(StepArgsError::count_mismatch(
                Self::FIELD_COUNT,
                captures.len(),
            ));
        }
        for (index, capture) in captures.iter().enumerate() {
            if !Self::FIELD_NAMES.contains(&capture.name) {
                return Err(StepArgsError::unconsumed_capture(capture.name));
            }
            if captures
                .iter()
                .take(index)
                .any(|earlier| earlier.name == capture.name)
            {
                return Err(StepArgsError::duplicate_capture(capture.name));
            }
        }
        let mut captures = captures;
        let values = Self::FIELD_NAMES
            .iter()
            .map(|field_name| {
                let index = captures
                    .iter()
                    .position(|capture| capture.name == *field_name)
                    .ok_or_else(|| StepArgsError::missing_field(field_name, field_name))?;
                Ok(captures.swap_remove(index).value)
            })
            .collect::<Result<Vec<_>, StepArgsError>>()?;
        Self::from_captures(values)
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for step argument extraction.

    use super::{StepArgs, StepArgsError, StepCapture};

    #[derive(Debug, PartialEq, Eq)]
    struct ManualArgs(Vec<String>);

    impl StepArgs for ManualArgs {
        const FIELD_COUNT: usize = 2;
        const FIELD_NAMES: &'static [&'static str] = &["first", "second"];

        fn from_captures(values: Vec<String>) -> Result<Self, StepArgsError> { Ok(Self(values)) }
    }

    #[test]
    fn parse_failure_formats_message() {
        let err = StepArgsError::parse_failure("count", "NaN");
        assert_eq!(
            err.to_string(),
            "failed to parse field 'count' from value 'NaN'"
        );
    }

    #[test]
    fn count_mismatch_formats_message() {
        let err = StepArgsError::count_mismatch(2, 1);
        assert_eq!(
            err.to_string(),
            "expected 2 captured value(s) but received 1"
        );
    }

    #[test]
    fn custom_parse_failure_preserves_parser_message() {
        let err = StepArgsError::custom_parse_failure("amount", "invalid", "not money");
        assert_eq!(
            err.to_string(),
            "failed to parse field 'amount' from value 'invalid': not money"
        );
    }

    #[test]
    fn manual_implementation_receives_values_in_field_order() {
        let args = ManualArgs::from_named_captures(vec![
            StepCapture {
                name: "second",
                value: String::from("two"),
            },
            StepCapture {
                name: "first",
                value: String::from("one"),
            },
        ])
        .expect("valid named captures should be reordered");

        assert_eq!(
            args,
            ManualArgs(vec![String::from("one"), String::from("two")])
        );
    }

    #[test]
    fn manual_implementation_rejects_duplicate_capture_names() {
        let error = ManualArgs::from_named_captures(vec![
            StepCapture {
                name: "first",
                value: String::from("one"),
            },
            StepCapture {
                name: "first",
                value: String::from("another"),
            },
        ])
        .expect_err("duplicate named captures should fail");

        assert_eq!(error, StepArgsError::duplicate_capture("first"));
    }
}
