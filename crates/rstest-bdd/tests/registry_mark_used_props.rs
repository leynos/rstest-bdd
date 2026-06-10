//! Property-based tests for the registry usage-marking invariant.
//!
//! Every public lookup variant funnels through the canonical
//! `mark_and_project` helper, so a lookup that returns `Some` must mark
//! exactly the resolved step as used, and a lookup that returns `None` must
//! mark nothing. This suite drives all six lookup variants (plus
//! `find_step_with_metadata`) against a pool of registered steps and asserts
//! the invariant via `unused_steps`.

use proptest::prelude::*;
use rstest_bdd::{
    StepContext, StepError, StepExecution, StepKeyword, find_step, find_step_async,
    find_step_async_with_mode, find_step_with_metadata, lookup_step, lookup_step_async,
    lookup_step_async_with_mode, step, unused_steps,
};

#[expect(
    clippy::unnecessary_wraps,
    reason = "step handlers must return Result<StepExecution, StepError>"
)]
fn noop_step_wrapper(
    _ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    Ok(StepExecution::from_value(None))
}

/// Patterns for steps the property may successfully look up.
const TARGET_PATTERNS: [&str; 3] = [
    "mark-used prop target alpha",
    "mark-used prop target beta",
    "mark-used prop target gamma",
];

/// Patterns for sentinel steps that are never looked up successfully; they
/// must remain unused throughout, proving failed lookups mark nothing.
const SENTINEL_PATTERNS: [&str; 2] = ["mark-used prop sentinel one", "mark-used prop sentinel two"];

step!(
    StepKeyword::Given,
    "mark-used prop target alpha",
    noop_step_wrapper,
    &[]
);
step!(
    StepKeyword::When,
    "mark-used prop target beta",
    noop_step_wrapper,
    &[]
);
step!(
    StepKeyword::Then,
    "mark-used prop target gamma",
    noop_step_wrapper,
    &[]
);
step!(
    StepKeyword::Given,
    "mark-used prop sentinel one",
    noop_step_wrapper,
    &[]
);
step!(
    StepKeyword::When,
    "mark-used prop sentinel two",
    noop_step_wrapper,
    &[]
);

/// Keyword each target pattern was registered under.
fn keyword_for(index: usize) -> StepKeyword {
    match index {
        0 => StepKeyword::Given,
        1 => StepKeyword::When,
        _ => StepKeyword::Then,
    }
}

/// Return whether the registry currently reports `pattern` as unused.
fn is_unused(pattern: &str) -> bool {
    unused_steps()
        .iter()
        .any(|step| step.pattern.as_str() == pattern)
}

/// Invoke one lookup variant, returning whether it resolved a step.
fn run_variant(variant: usize, keyword: StepKeyword, text: &str) -> bool {
    match variant {
        0 => lookup_step(keyword, text.into()).is_some(),
        1 => find_step(keyword, text.into()).is_some(),
        2 => lookup_step_async(keyword, text.into()).is_some(),
        3 => find_step_async(keyword, text.into()).is_some(),
        4 => lookup_step_async_with_mode(keyword, text.into()).is_some(),
        5 => find_step_async_with_mode(keyword, text.into()).is_some(),
        _ => find_step_with_metadata(keyword, text.into()).is_some(),
    }
}

proptest! {
    /// A successful lookup through any variant marks the resolved step used;
    /// a failed lookup marks nothing (sentinels stay unused throughout).
    #[test]
    fn every_lookup_variant_upholds_usage_marking(
        variant in 0usize..7,
        target in 0usize..TARGET_PATTERNS.len(),
        hit in any::<bool>(),
        miss_suffix in "[a-z]{4,12}",
    ) {
        let pattern = TARGET_PATTERNS
            .get(target)
            .ok_or_else(|| TestCaseError::fail("target index in range"))?;
        let keyword = keyword_for(target);

        if hit {
            let resolved = run_variant(variant, keyword, pattern);
            prop_assert!(resolved, "registered step must resolve");
            prop_assert!(
                !is_unused(pattern),
                "successful lookup must mark the step used"
            );
        } else {
            let missing = format!("mark-used prop missing {miss_suffix}");
            let resolved = run_variant(variant, keyword, &missing);
            prop_assert!(!resolved, "unregistered text must not resolve");
        }

        for sentinel in SENTINEL_PATTERNS {
            prop_assert!(
                is_unused(sentinel),
                "failed lookups must not mark sentinel steps used"
            );
        }
    }
}
