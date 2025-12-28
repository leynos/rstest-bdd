//! Domain types for scenario code generation.
//!
//! These types provide semantic wrappers around string-based data structures
//! to improve code clarity and type safety in scenario code generation.

/// Wraps step text content.
#[derive(Debug, Clone)]
pub(crate) struct StepText(String);

impl StepText {
    /// Creates a new step text wrapper.
    pub(crate) fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// Returns the step text as a string slice.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for StepText {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Wraps Examples table column names.
#[derive(Debug, Clone)]
pub(crate) struct ExampleHeaders(Vec<String>);

impl ExampleHeaders {
    /// Creates a new headers wrapper.
    pub(crate) fn new(headers: Vec<String>) -> Self {
        Self(headers)
    }

    /// Returns the headers as a slice.
    pub(crate) fn as_slice(&self) -> &[String] {
        &self.0
    }
}

impl AsRef<[String]> for ExampleHeaders {
    fn as_ref(&self) -> &[String] {
        self.as_slice()
    }
}

/// Wraps Examples table row values.
#[derive(Debug, Clone)]
pub(crate) struct ExampleRow(Vec<String>);

impl ExampleRow {
    /// Creates a new row wrapper.
    pub(crate) fn new(row: Vec<String>) -> Self {
        Self(row)
    }

    /// Returns the row values as a slice.
    pub(crate) fn as_slice(&self) -> &[String] {
        &self.0
    }
}

impl AsRef<[String]> for ExampleRow {
    fn as_ref(&self) -> &[String] {
        self.as_slice()
    }
}

/// Wraps multiline documentation strings.
#[derive(Debug, Clone)]
pub(crate) struct Docstring(String);

impl Docstring {
    /// Creates a new docstring wrapper.
    pub(crate) fn new(content: impl Into<String>) -> Self {
        Self(content.into())
    }

    /// Returns the docstring as a string slice.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Docstring {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
