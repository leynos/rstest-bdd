//! Shared execution policy types for rstest-bdd.
//!
//! This crate centralizes runtime policy enums so both the runtime crate and
//! the proc-macro crate can depend on a single, canonical definition without
//! creating a proc-macro dependency cycle.
//!
//! It also provides canonical attribute-policy path resolution helpers used by
//! macro codegen.

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
    /// Generate `#[rstest::rstest]` and `#[gpui::test]`.
    RstestWithGpuiTest,
}

/// Canonical path segments for `DefaultAttributePolicy`.
pub const DEFAULT_ATTRIBUTE_POLICY_PATH: &[&str] =
    &["rstest_bdd_harness", "DefaultAttributePolicy"];

/// Canonical path segments for `StdHarness`.
pub const STD_HARNESS_PATH: &[&str] = &["rstest_bdd_harness", "StdHarness"];

/// Canonical path segments for `TokioAttributePolicy`.
pub const TOKIO_ATTRIBUTE_POLICY_PATH: &[&str] =
    &["rstest_bdd_harness_tokio", "TokioAttributePolicy"];

/// Canonical path segments for `TokioHarness`.
pub const TOKIO_HARNESS_PATH: &[&str] = &["rstest_bdd_harness_tokio", "TokioHarness"];

/// Canonical path segments for `GpuiAttributePolicy`.
pub const GPUI_ATTRIBUTE_POLICY_PATH: &[&str] = &["rstest_bdd_harness_gpui", "GpuiAttributePolicy"];

/// Canonical path segments for `GpuiHarness`.
pub const GPUI_HARNESS_PATH: &[&str] = &["rstest_bdd_harness_gpui", "GpuiHarness"];

const KNOWN_ATTRIBUTE_POLICY_HINTS: [(&[&str], TestAttributeHint); 3] = [
    (DEFAULT_ATTRIBUTE_POLICY_PATH, TestAttributeHint::RstestOnly),
    (
        TOKIO_ATTRIBUTE_POLICY_PATH,
        TestAttributeHint::RstestWithTokioCurrentThread,
    ),
    (
        GPUI_ATTRIBUTE_POLICY_PATH,
        TestAttributeHint::RstestWithGpuiTest,
    ),
];

const KNOWN_HARNESS_HINTS: [(&[&str], TestAttributeHint); 3] = [
    (STD_HARNESS_PATH, TestAttributeHint::RstestOnly),
    (
        TOKIO_HARNESS_PATH,
        TestAttributeHint::RstestWithTokioCurrentThread,
    ),
    (GPUI_HARNESS_PATH, TestAttributeHint::RstestWithGpuiTest),
];

/// Resolves a canonical attribute policy path into a test-attribute hint.
///
/// Path segments should be provided without a leading `::`.
///
/// # Examples
///
/// ```
/// use rstest_bdd_policy::{
///     resolve_test_attribute_hint_for_policy_path, TestAttributeHint,
/// };
///
/// assert_eq!(
///     resolve_test_attribute_hint_for_policy_path(&[
///         "rstest_bdd_harness_tokio",
///         "TokioAttributePolicy",
///     ]),
///     Some(TestAttributeHint::RstestWithTokioCurrentThread)
/// );
/// assert_eq!(
///     resolve_test_attribute_hint_for_policy_path(&["my", "Policy"]),
///     None
/// );
/// ```
#[must_use]
pub fn resolve_test_attribute_hint_for_policy_path(
    path_segments: &[&str],
) -> Option<TestAttributeHint> {
    KNOWN_ATTRIBUTE_POLICY_HINTS
        .iter()
        .find_map(|(known_path, hint)| (path_segments == *known_path).then_some(*hint))
}

/// Resolves a canonical harness path into a test-attribute hint.
///
/// Path segments should be provided without a leading `::`.
///
/// # Examples
///
/// ```
/// use rstest_bdd_policy::{
///     resolve_test_attribute_hint_for_harness_path, TestAttributeHint,
/// };
///
/// assert_eq!(
///     resolve_test_attribute_hint_for_harness_path(&[
///         "rstest_bdd_harness_tokio",
///         "TokioHarness",
///     ]),
///     Some(TestAttributeHint::RstestWithTokioCurrentThread)
/// );
/// assert_eq!(
///     resolve_test_attribute_hint_for_harness_path(&["my", "Harness"]),
///     None
/// );
/// ```
#[must_use]
pub fn resolve_test_attribute_hint_for_harness_path(
    path_segments: &[&str],
) -> Option<TestAttributeHint> {
    KNOWN_HARNESS_HINTS
        .iter()
        .find_map(|(known_path, hint)| (path_segments == *known_path).then_some(*hint))
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_ATTRIBUTE_POLICY_PATH, GPUI_ATTRIBUTE_POLICY_PATH, GPUI_HARNESS_PATH, RuntimeMode,
        STD_HARNESS_PATH, TOKIO_ATTRIBUTE_POLICY_PATH, TOKIO_HARNESS_PATH, TestAttributeHint,
        resolve_test_attribute_hint_for_harness_path, resolve_test_attribute_hint_for_policy_path,
    };

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

    #[test]
    fn resolves_default_attribute_policy_path() {
        assert_eq!(
            resolve_test_attribute_hint_for_policy_path(DEFAULT_ATTRIBUTE_POLICY_PATH),
            Some(TestAttributeHint::RstestOnly)
        );
    }

    #[test]
    fn resolves_tokio_attribute_policy_path() {
        assert_eq!(
            resolve_test_attribute_hint_for_policy_path(TOKIO_ATTRIBUTE_POLICY_PATH),
            Some(TestAttributeHint::RstestWithTokioCurrentThread)
        );
    }

    #[test]
    fn resolves_gpui_attribute_policy_path() {
        assert_eq!(
            resolve_test_attribute_hint_for_policy_path(GPUI_ATTRIBUTE_POLICY_PATH),
            Some(TestAttributeHint::RstestWithGpuiTest)
        );
    }

    #[test]
    fn unknown_attribute_policy_path_returns_none() {
        assert_eq!(
            resolve_test_attribute_hint_for_policy_path(&["my", "Policy"]),
            None
        );
    }

    #[test]
    fn partial_attribute_policy_path_returns_none() {
        assert_eq!(
            resolve_test_attribute_hint_for_policy_path(&["TokioAttributePolicy"]),
            None
        );
    }

    #[test]
    fn resolves_std_harness_path() {
        assert_eq!(
            resolve_test_attribute_hint_for_harness_path(STD_HARNESS_PATH),
            Some(TestAttributeHint::RstestOnly)
        );
    }

    #[test]
    fn resolves_tokio_harness_path() {
        assert_eq!(
            resolve_test_attribute_hint_for_harness_path(TOKIO_HARNESS_PATH),
            Some(TestAttributeHint::RstestWithTokioCurrentThread)
        );
    }

    #[test]
    fn resolves_gpui_harness_path() {
        assert_eq!(
            resolve_test_attribute_hint_for_harness_path(GPUI_HARNESS_PATH),
            Some(TestAttributeHint::RstestWithGpuiTest)
        );
    }

    #[test]
    fn unknown_harness_path_returns_none() {
        assert_eq!(
            resolve_test_attribute_hint_for_harness_path(&["my", "Harness"]),
            None
        );
    }

    #[test]
    fn partial_harness_path_returns_none() {
        assert_eq!(
            resolve_test_attribute_hint_for_harness_path(&["TokioHarness"]),
            None
        );
    }
}
