//! Attribute policy plug-ins for generated scenario tests.

/// A single test attribute emitted by an [`AttributePolicy`].
///
/// Values are stored as path + optional argument payload so the macro layer can
/// turn them into concrete attributes during expansion.
///
/// # Examples
///
/// ```
/// use rstest_bdd_harness::TestAttribute;
///
/// let rstest = TestAttribute::new("rstest::rstest");
/// assert_eq!(rstest.render(), "#[rstest::rstest]");
///
/// let tokio = TestAttribute::with_arguments("tokio::test", "flavor = \"current_thread\"");
/// assert_eq!(tokio.render(), "#[tokio::test(flavor = \"current_thread\")]");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TestAttribute {
    path: &'static str,
    arguments: Option<&'static str>,
}

impl TestAttribute {
    /// Creates an attribute with no argument list.
    #[must_use]
    pub const fn new(path: &'static str) -> Self {
        Self {
            path,
            arguments: None,
        }
    }

    /// Creates an attribute with a parenthesized argument list.
    #[must_use]
    pub const fn with_arguments(path: &'static str, arguments: &'static str) -> Self {
        Self {
            path,
            arguments: Some(arguments),
        }
    }

    /// Returns the attribute path.
    #[must_use]
    pub const fn path(self) -> &'static str {
        self.path
    }

    /// Returns optional argument payload.
    #[must_use]
    pub const fn arguments(self) -> Option<&'static str> {
        self.arguments
    }

    /// Renders the attribute as text for diagnostics and tests.
    #[must_use]
    pub fn render(self) -> String {
        self.arguments.map_or_else(
            || format!("#[{}]", self.path),
            |arguments| format!("#[{}({arguments})]", self.path),
        )
    }
}

/// Supplies test attributes for generated scenario functions.
///
/// # Examples
///
/// ```
/// use rstest_bdd_harness::{AttributePolicy, DefaultAttributePolicy};
///
/// let attributes = DefaultAttributePolicy::test_attributes();
/// assert_eq!(attributes.len(), 1);
/// assert_eq!(attributes[0].render(), "#[rstest::rstest]");
/// ```
pub trait AttributePolicy {
    /// Returns attributes in the order they should be applied.
    fn test_attributes() -> &'static [TestAttribute];
}

/// Default attribute policy used by the standard harness.
///
/// This policy emits only `#[rstest::rstest]`.
pub struct DefaultAttributePolicy;

const DEFAULT_TEST_ATTRIBUTES: [TestAttribute; 1] = [TestAttribute::new("rstest::rstest")];

impl AttributePolicy for DefaultAttributePolicy {
    fn test_attributes() -> &'static [TestAttribute] {
        &DEFAULT_TEST_ATTRIBUTES
    }
}

#[cfg(test)]
mod tests {
    use super::{AttributePolicy, DefaultAttributePolicy, TestAttribute};

    #[test]
    fn test_attribute_render_without_arguments() {
        let attribute = TestAttribute::new("rstest::rstest");
        assert_eq!(attribute.render(), "#[rstest::rstest]");
        assert_eq!(attribute.path(), "rstest::rstest");
        assert_eq!(attribute.arguments(), None);
    }

    #[test]
    fn test_attribute_render_with_arguments() {
        let attribute = TestAttribute::with_arguments("tokio::test", "flavor = \"current_thread\"");
        assert_eq!(
            attribute.render(),
            "#[tokio::test(flavor = \"current_thread\")]"
        );
        assert_eq!(attribute.path(), "tokio::test");
        assert_eq!(attribute.arguments(), Some("flavor = \"current_thread\""));
    }

    #[test]
    fn default_policy_emits_only_rstest() {
        let attributes = DefaultAttributePolicy::test_attributes();
        assert_eq!(attributes, [TestAttribute::new("rstest::rstest")]);
    }
}
