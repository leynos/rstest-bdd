//! Property-based tests for the registry usage-marking invariant.
//!
//! Every public lookup variant funnels through the canonical
//! `mark_and_project` helper, so a lookup that returns `Some` must mark
//! exactly the resolved step as used, and a lookup that returns `None` must
//! mark nothing. This suite drives all six lookup variants (plus
//! `find_step_with_metadata`) against a pool of registered steps and asserts
//! the invariant via `unused_steps`.

use proptest::prelude::*;
use std::collections::BTreeSet;

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

/// Patterns for every step whose usage state is asserted by this suite.
const OBSERVED_PATTERNS: [&str; 5] = [
    "mark-used prop target alpha",
    "mark-used prop target beta",
    "mark-used prop target gamma",
    "mark-used prop sentinel one",
    "mark-used prop sentinel two",
];

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

/// Keyword each sentinel pattern was registered under.
fn sentinel_keyword_for(index: usize) -> StepKeyword {
    match index {
        0 => StepKeyword::Given,
        _ => StepKeyword::When,
    }
}

/// Return a keyword that is guaranteed to differ from the given keyword.
fn mismatched_keyword(keyword: StepKeyword) -> StepKeyword {
    match keyword {
        StepKeyword::Given => StepKeyword::When,
        StepKeyword::When => StepKeyword::Then,
        StepKeyword::Then | StepKeyword::And | StepKeyword::But => StepKeyword::Given,
    }
}

/// Return the observed patterns the registry currently reports as unused.
fn unused_observed_patterns() -> BTreeSet<&'static str> {
    unused_steps()
        .iter()
        .filter_map(|step| {
            OBSERVED_PATTERNS
                .iter()
                .copied()
                .find(|pattern| step.pattern.as_str() == *pattern)
        })
        .collect()
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
            let before_unused = unused_observed_patterns();
            let resolved = run_variant(variant, keyword, pattern);
            prop_assert!(resolved, "registered step must resolve");
            let after_unused = unused_observed_patterns();
            let newly_used = before_unused
                .difference(&after_unused)
                .copied()
                .collect::<Vec<_>>();

            prop_assert!(
                newly_used.iter().all(|used_pattern| *used_pattern == *pattern),
                "successful lookup marked unrelated steps used: {:?}",
                newly_used
            );
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

    /// A lookup whose text matches a registered pattern but whose keyword does
    /// not match must behave as a miss and leave the step unused.
    #[test]
    fn mismatched_keyword_lookups_do_not_resolve_or_mark_used(
        variant in 0usize..7,
        sentinel in 0usize..SENTINEL_PATTERNS.len(),
    ) {
        let pattern = SENTINEL_PATTERNS
            .get(sentinel)
            .ok_or_else(|| TestCaseError::fail("sentinel index in range"))?;
        let keyword = sentinel_keyword_for(sentinel);
        let wrong_keyword = mismatched_keyword(keyword);

        let resolved = run_variant(variant, wrong_keyword, pattern);
        prop_assert!(
            !resolved,
            "lookup unexpectedly resolved step with mismatched keyword: {:?} vs {:?}",
            wrong_keyword,
            keyword
        );
        prop_assert!(
            is_unused(pattern),
            "lookup with mismatched keyword marked pattern as used: {pattern}"
        );
    }
}
