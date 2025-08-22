//! Step registration and lookup.
//! This module defines the `Step` record, the `step!` macro for registration,
//! and the global registry used to find steps by keyword and pattern or by
//! placeholder matching.

use crate::pattern::StepPattern;
use crate::placeholder::extract_placeholders;
use crate::types::{PatternStr, StepFn, StepKeyword, StepText};
use hashbrown::HashMap;
use inventory::iter;
use std::hash::{BuildHasher, Hash};
use std::sync::LazyLock;

/// Represents a single step definition registered with the framework.
#[derive(Debug)]
pub struct Step {
    /// The step keyword, e.g. `Given` or `When`.
    pub keyword: StepKeyword,
    /// Pattern text used to match a Gherkin step.
    pub pattern: &'static StepPattern,
    /// Function pointer executed when the step is invoked.
    pub run: StepFn,
    /// Names of fixtures this step requires.
    pub fixtures: &'static [&'static str],
    /// Source file where the step is defined.
    pub file: &'static str,
    /// Line number within the source file.
    pub line: u32,
}

/// Register a step definition with the global registry.
#[macro_export]
macro_rules! step {
    (@pattern $keyword:expr, $pattern:expr, $handler:path, $fixtures:expr) => {
        const _: () = {
            $crate::submit! {
                $crate::Step {
                    keyword: $keyword,
                    pattern: $pattern,
                    run: $handler,
                    fixtures: $fixtures,
                    file: file!(),
                    line: line!(),
                }
            }
        };
    };

    ($keyword:expr, $pattern:expr, $handler:path, $fixtures:expr) => {
        const _: () = {
            static PATTERN: $crate::StepPattern = $crate::StepPattern::new($pattern);
    $crate::step!(@pattern $keyword, &PATTERN, $handler, $fixtures);
        };
    };
}

inventory::collect!(Step);

type StepKey = (StepKeyword, &'static StepPattern);

static STEP_MAP: LazyLock<HashMap<StepKey, StepFn>> = LazyLock::new(|| {
    let steps: Vec<_> = iter::<Step>.into_iter().collect();
    let mut map = HashMap::with_capacity(steps.len());
    for step in steps {
        step.pattern.compile().unwrap_or_else(|e| {
            panic!(
                "invalid step pattern '{}' at {}:{}: {e}",
                step.pattern.as_str(),
                step.file,
                step.line
            )
        });
        let key = (step.keyword, step.pattern);
        if map.contains_key(&key) {
            panic!(
                "duplicate step for '{}' + '{}' defined at {}:{}",
                step.keyword.as_str(),
                step.pattern.as_str(),
                step.file,
                step.line
            );
        }
        map.insert(key, step.run);
    }
    map
});

/// Look up a registered step by keyword and pattern.
#[must_use]
pub fn lookup_step(keyword: StepKeyword, pattern: PatternStr<'_>) -> Option<StepFn> {
    // Compute the hash as if the key were (keyword, pattern.as_str())
    // because StepPattern hashing is by its inner text.
    let build = STEP_MAP.hasher();
    let mut state = build.build_hasher();
    keyword.hash(&mut state);
    pattern.as_str().hash(&mut state);
    let hash = state.finish();

    STEP_MAP
        .raw_entry()
        .from_hash(hash, |(kw, pat)| *kw == keyword && pat.as_str() == pattern.as_str())
        .map(|(_, &f)| f)
}

/// Find a registered step whose pattern matches the provided text.
#[must_use]
pub fn find_step(keyword: StepKeyword, text: StepText<'_>) -> Option<StepFn> {
    if let Some(f) = lookup_step(keyword, text.as_str().into()) {
        return Some(f);
    }
    for step in iter::<Step> {
        if step.keyword == keyword && extract_placeholders(step.pattern, text).is_ok() {
            return Some(step.run);
        }
    }
    None
}
