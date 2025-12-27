//! Functions for recording bypassed steps during scenario skips.

use crate::types::StepKeyword;

/// Record step definitions that were bypassed after a scenario requested a skip.
///
/// This is a no-op when the `diagnostics` feature is disabled so that generated
/// test code can reference this function unconditionally without breaking
/// `default-features = false` builds.
pub fn record_bypassed_steps<'a, I>(
    feature_path: impl Into<String>,
    scenario_name: impl Into<String>,
    scenario_line: u32,
    tags: impl Into<Vec<String>>,
    reason: Option<&str>,
    steps: I,
) where
    I: IntoIterator<Item = (StepKeyword, &'a str)>,
{
    #[cfg(feature = "diagnostics")]
    {
        let feature_path = feature_path.into();
        let scenario_name = scenario_name.into();
        let tags = tags.into();
        super::diagnostics::record_bypassed_steps_impl(
            feature_path.as_str(),
            scenario_name.as_str(),
            scenario_line,
            &tags,
            reason,
            steps,
        );
    }

    #[cfg(not(feature = "diagnostics"))]
    {
        let _ = (
            feature_path,
            scenario_name,
            scenario_line,
            tags,
            reason,
            steps,
        );
    }
}

/// Record bypassed steps using previously owned tags.
///
/// Generated scenario code often already owns a `Vec<String>` for reporting.
/// Borrowing it here avoids an additional `Vec<String>` clone at call sites
/// whilst still behaving as a no-op when diagnostics are disabled.
pub fn record_bypassed_steps_with_tags<'a, I>(
    feature_path: impl Into<String>,
    scenario_name: impl Into<String>,
    scenario_line: u32,
    tags: &[String],
    reason: Option<&str>,
    steps: I,
) where
    I: IntoIterator<Item = (StepKeyword, &'a str)>,
{
    #[cfg(feature = "diagnostics")]
    {
        let feature_path = feature_path.into();
        let scenario_name = scenario_name.into();
        super::diagnostics::record_bypassed_steps_impl(
            feature_path.as_str(),
            scenario_name.as_str(),
            scenario_line,
            tags,
            reason,
            steps,
        );
    }

    #[cfg(not(feature = "diagnostics"))]
    {
        let _ = (
            feature_path,
            scenario_name,
            scenario_line,
            tags,
            reason,
            steps,
        );
    }
}
