//! Scenario metadata wrappers shared across macro code generation.
use std::fmt;

/// Path to a feature file on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FeaturePath(String);

impl FeaturePath {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<String> for FeaturePath {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for FeaturePath {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl AsRef<str> for FeaturePath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for FeaturePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Name of an individual Gherkin scenario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScenarioName(String);

impl ScenarioName {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<String> for ScenarioName {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for ScenarioName {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl AsRef<str> for ScenarioName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ScenarioName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
