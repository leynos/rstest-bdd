//! Shared execution policy types for rstest-bdd.
//!
//! This crate centralises runtime policy enums so both the runtime crate and
//! the proc-macro crate can depend on a single, canonical definition without
//! creating a proc-macro dependency cycle.

/// Runtime mode for scenario test execution.
///
/// # Examples
///
/// ```
/// use rstest_bdd_policy::RuntimeMode;
///
/// let mode = RuntimeMode::default();
/// assert!(!mode.is_async());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RuntimeMode {
    /// Synchronous execution (default).
    #[default]
    Sync,
    /// Tokio current-thread runtime (`#[tokio::test(flavor = "current_thread")]`).
    TokioCurrentThread,
}

impl RuntimeMode {
    /// Returns `true` if this mode requires async test generation.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd_policy::RuntimeMode;
    ///
    /// assert!(!RuntimeMode::Sync.is_async());
    /// assert!(RuntimeMode::TokioCurrentThread.is_async());
    /// ```
    #[must_use]
    pub const fn is_async(self) -> bool {
        matches!(self, Self::TokioCurrentThread)
    }

    /// Returns a hint for which test attributes to generate.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd_policy::{RuntimeMode, TestAttributeHint};
    ///
    /// assert_eq!(
    ///     RuntimeMode::Sync.test_attribute_hint(),
    ///     TestAttributeHint::RstestOnly
    /// );
    /// assert_eq!(
    ///     RuntimeMode::TokioCurrentThread.test_attribute_hint(),
    ///     TestAttributeHint::RstestWithTokioCurrentThread
    /// );
    /// ```
    #[must_use]
    pub const fn test_attribute_hint(self) -> TestAttributeHint {
        match self {
            Self::Sync => TestAttributeHint::RstestOnly,
            Self::TokioCurrentThread => TestAttributeHint::RstestWithTokioCurrentThread,
        }
    }
}

/// Hint for which test attributes the macro layer should generate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestAttributeHint {
    /// Generate only `#[rstest::rstest]`.
    RstestOnly,
    /// Generate `#[rstest::rstest]` and `#[tokio::test(flavor = "current_thread")]`.
    RstestWithTokioCurrentThread,
}

#[cfg(test)]
mod tests {
    use super::{RuntimeMode, TestAttributeHint};

    #[test]
    fn runtime_mode_sync_is_default() {
        assert_eq!(RuntimeMode::default(), RuntimeMode::Sync);
    }

    #[test]
    fn runtime_mode_sync_is_not_async() {
        assert!(!RuntimeMode::Sync.is_async());
    }

    #[test]
    fn runtime_mode_tokio_current_thread_is_async() {
        assert!(RuntimeMode::TokioCurrentThread.is_async());
    }

    #[test]
    fn runtime_mode_sync_hint_is_rstest_only() {
        assert_eq!(
            RuntimeMode::Sync.test_attribute_hint(),
            TestAttributeHint::RstestOnly
        );
    }

    #[test]
    fn runtime_mode_tokio_hint_is_rstest_with_tokio() {
        assert_eq!(
            RuntimeMode::TokioCurrentThread.test_attribute_hint(),
            TestAttributeHint::RstestWithTokioCurrentThread
        );
    }
}
