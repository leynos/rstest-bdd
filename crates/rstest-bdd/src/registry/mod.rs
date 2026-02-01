//! Step registration and lookup.
//! This module defines the `Step` record, the `step!` macro for registration,
//! and the global registry used to find steps by keyword and pattern or by
//! placeholder matching.

use crate::pattern::StepPattern;
use crate::placeholder::extract_placeholders;
use crate::types::{AsyncStepFn, PatternStr, StepExecutionMode, StepFn, StepKeyword, StepText};
use hashbrown::{HashMap, HashSet};
use inventory::iter;
use rstest_bdd_patterns::SpecificityScore;
use std::hash::{BuildHasher, Hash, Hasher};
use std::sync::{LazyLock, Mutex};

mod async_lookup;
mod bypassed;
#[cfg(feature = "diagnostics")]
pub(crate) mod diagnostics;

pub use async_lookup::{
    find_step_async_with_mode, find_step_with_mode, lookup_step_async_with_mode,
};
pub use bypassed::{record_bypassed_steps, record_bypassed_steps_with_tags};

/// Represents a single step definition registered with the framework.
#[derive(Debug)]
pub struct Step {
    /// The step keyword, e.g. `Given` or `When`.
    pub keyword: StepKeyword,
    /// Pattern text used to match a Gherkin step.
    pub pattern: &'static StepPattern,
    /// Function pointer executed when the step is invoked (sync mode).
    pub run: StepFn,
    /// Function pointer executed when the step is invoked (async mode).
    ///
    /// For sync step definitions, this wraps the result in an immediately-ready
    /// future, enabling mixed sync and async steps within async scenarios.
    pub run_async: AsyncStepFn,
    /// Whether the step has a native sync body, a native async body, or both.
    pub execution_mode: StepExecutionMode,
    /// Names of fixtures this step requires.
    pub fixtures: &'static [&'static str],
    /// Source file where the step is defined.
    pub file: &'static str,
    /// Line number within the source file.
    pub line: u32,
}

/// Register a step definition with the global registry.
///
/// This macro accepts both sync and async handler function pointers. The async
/// handler wraps the sync result in an immediately-ready future for sync step
/// definitions, enabling unified execution in async scenarios.
///
/// # Forms
///
/// The macro supports two forms:
///
/// ## 5-argument form (explicit async handler)
///
/// ```ignore
/// step!(keyword, pattern, sync_handler, async_handler, fixtures);
/// // With explicit execution mode:
/// step!(
///     keyword,
///     pattern,
///     sync_handler,
///     async_handler,
///     fixtures,
///     mode = StepExecutionMode::Async
/// );
/// ```
///
/// ## 4-argument form (auto-generated async handler)
///
/// ```ignore
/// step!(keyword, pattern, sync_handler, fixtures);
/// // With explicit execution mode:
/// step!(keyword, pattern, sync_handler, fixtures, mode = StepExecutionMode::Sync);
/// ```
///
/// The 4-argument form automatically generates an async wrapper that delegates
/// to the sync handler via an immediately-ready future. This provides backward
/// compatibility for call sites that only define sync handlers.
///
/// When the `mode` parameter is omitted, both forms default to
/// [`StepExecutionMode::Both`].
#[macro_export]
macro_rules! step {
    // Internal arm: 5 arguments with pre-compiled pattern reference
    (@pattern $keyword:expr, $pattern:expr, $handler:path, $async_handler:path, $fixtures:expr, $mode:expr) => {
        const _: () = {
            $crate::submit! {
                $crate::Step {
                    keyword: $keyword,
                    pattern: $pattern,
                    run: $handler,
                    run_async: $async_handler,
                    execution_mode: $mode,
                    fixtures: $fixtures,
                    file: file!(),
                    line: line!(),
                }
            }
        };
    };

    // Internal arm: 4 arguments with pre-compiled pattern reference (auto-generate async)
    (@pattern $keyword:expr, $pattern:expr, $handler:path, $fixtures:expr, $mode:expr) => {
        const _: () = {
            fn __rstest_bdd_auto_async<'ctx, 'fixtures>(
                ctx: &'ctx mut $crate::StepContext<'fixtures>,
                text: &'ctx str,
                docstring: ::core::option::Option<&'ctx str>,
                table: ::core::option::Option<&'ctx [&'ctx [&'ctx str]]>,
            ) -> $crate::StepFuture<'ctx> {
                ::std::boxed::Box::pin(::std::future::ready($handler(ctx, text, docstring, table)))
            }

            $crate::submit! {
                $crate::Step {
                    keyword: $keyword,
                    pattern: $pattern,
                    run: $handler,
                    run_async: __rstest_bdd_auto_async,
                    execution_mode: $mode,
                    fixtures: $fixtures,
                    file: file!(),
                    line: line!(),
                }
            }
        };
    };

    // Public arm: 4 arguments (auto-generate async handler for backward compatibility)
    // This arm MUST come before the 5-argument arm because Rust macro matching
    // is greedy and would otherwise try to parse fixtures as an async_handler path.
    ($keyword:expr, $pattern:expr, $handler:path, & $fixtures:expr, mode = $mode:expr $(,)?) => {
        const _: () = {
            static PATTERN: $crate::StepPattern = $crate::StepPattern::new($pattern);
    $crate::step!(@pattern $keyword, &PATTERN, $handler, &$fixtures, $mode);
        };
    };

    // Public arm: 4 arguments defaulting to Both.
    ($keyword:expr, $pattern:expr, $handler:path, & $fixtures:expr) => {
        const _: () = {
            static PATTERN: $crate::StepPattern = $crate::StepPattern::new($pattern);
            $crate::step!(
                @pattern $keyword,
                &PATTERN,
                $handler,
                &$fixtures,
                $crate::StepExecutionMode::Both
            );
        };
    };

    // Public arm: 5 arguments (explicit async handler)
    ($keyword:expr, $pattern:expr, $handler:path, $async_handler:path, $fixtures:expr, mode = $mode:expr $(,)?) => {
        const _: () = {
            static PATTERN: $crate::StepPattern = $crate::StepPattern::new($pattern);
    $crate::step!(@pattern $keyword, &PATTERN, $handler, $async_handler, $fixtures, $mode);
        };
    };

    // Public arm: 5 arguments defaulting to Both.
    ($keyword:expr, $pattern:expr, $handler:path, $async_handler:path, $fixtures:expr) => {
        const _: () = {
            static PATTERN: $crate::StepPattern = $crate::StepPattern::new($pattern);
            $crate::step!(
                @pattern $keyword,
                &PATTERN,
                $handler,
                $async_handler,
                $fixtures,
                $crate::StepExecutionMode::Both
            );
        };
    };
}

inventory::collect!(Step);

type StepKey = (StepKeyword, &'static StepPattern);

static STEP_MAP: LazyLock<HashMap<StepKey, &'static Step>> = LazyLock::new(|| {
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
        assert!(
            !map.contains_key(&key),
            "duplicate step for '{}' + '{}' defined at {}:{}",
            step.keyword.as_str(),
            step.pattern.as_str(),
            step.file,
            step.line
        );
        map.insert(key, step);
    }
    map
});

// Tracks step invocations for the lifetime of the current process only. The
// data is not persisted across binaries, keeping usage bookkeeping lightweight
// and ephemeral.
static USED_STEPS: LazyLock<Mutex<HashSet<StepKey>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

fn mark_used(key: StepKey) {
    USED_STEPS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(key);
}

fn all_steps() -> Vec<&'static Step> {
    iter::<Step>.into_iter().collect()
}

fn step_by_key(key: StepKey) -> Option<&'static Step> {
    STEP_MAP.get(&key).copied()
}

fn resolve_exact_step(keyword: StepKeyword, pattern: PatternStr<'_>) -> Option<&'static Step> {
    // Compute the hash as if the key were (keyword, pattern.as_str()) because
    // StepPattern hashing is by its inner text.
    let build = STEP_MAP.hasher();
    let mut state = build.build_hasher();
    keyword.hash(&mut state);
    pattern.as_str().hash(&mut state);
    let hash = state.finish();

    STEP_MAP
        .raw_entry()
        .from_hash(hash, |(kw, pat)| {
            *kw == keyword && pat.as_str() == pattern.as_str()
        })
        .map(|(_, step)| *step)
}

fn resolve_step(keyword: StepKeyword, text: StepText<'_>) -> Option<&'static Step> {
    // Fast path: exact pattern match
    if let Some(step) = resolve_exact_step(keyword, text.as_str().into()) {
        return Some(step);
    }

    // Find the most specific matching step directly via iterator
    iter::<Step>
        .into_iter()
        .filter(|step| step.keyword == keyword && extract_placeholders(step.pattern, text).is_ok())
        .max_by(|a, b| {
            let a_spec = step_specificity(a);
            let b_spec = step_specificity(b);
            a_spec.cmp(&b_spec)
        })
}

/// Compute the specificity score for a step, logging any errors.
fn step_specificity(step: &Step) -> SpecificityScore {
    step.pattern.specificity().unwrap_or_else(|e| {
        log::warn!(
            "specificity calculation failed for pattern '{}': {e}",
            step.pattern.as_str()
        );
        SpecificityScore::default()
    })
}

/// Look up a registered step by keyword and pattern.
#[must_use]
pub fn lookup_step(keyword: StepKeyword, pattern: PatternStr<'_>) -> Option<StepFn> {
    resolve_exact_step(keyword, pattern).map(|step| {
        mark_used((step.keyword, step.pattern));
        step.run
    })
}

/// Find a registered step whose pattern matches the provided text.
#[must_use]
pub fn find_step(keyword: StepKeyword, text: StepText<'_>) -> Option<StepFn> {
    resolve_step(keyword, text).map(|step| {
        mark_used((step.keyword, step.pattern));
        step.run
    })
}

/// Look up a registered async step by keyword and pattern.
///
/// Returns the async step function pointer for use in async scenario execution.
/// The async wrapper returns an immediately-ready future for sync step
/// definitions.
#[must_use]
pub fn lookup_step_async(keyword: StepKeyword, pattern: PatternStr<'_>) -> Option<AsyncStepFn> {
    resolve_exact_step(keyword, pattern).map(|step| {
        mark_used((step.keyword, step.pattern));
        step.run_async
    })
}

/// Find a registered async step whose pattern matches the provided text.
///
/// Returns the async step function pointer for use in async scenario execution.
/// The async wrapper returns an immediately-ready future for sync step
/// definitions.
#[must_use]
pub fn find_step_async(keyword: StepKeyword, text: StepText<'_>) -> Option<AsyncStepFn> {
    resolve_step(keyword, text).map(|step| {
        mark_used((step.keyword, step.pattern));
        step.run_async
    })
}

/// Find a registered step and return its full metadata.
///
/// Unlike [`find_step`], this function returns the entire [`Step`] struct,
/// providing access to the step's required fixtures, source location, and
/// other metadata. This is useful for fixture validation and error reporting.
///
/// # Examples
///
/// ```ignore
/// use rstest_bdd::{find_step_with_metadata, StepKeyword, StepText};
///
/// if let Some(step) = find_step_with_metadata(StepKeyword::Given, StepText::from("a value")) {
///     println!("Step requires fixtures: {:?}", step.fixtures);
///     // Invoke the step function
///     let result = (step.run)(&mut ctx, text, None, None);
/// }
/// ```
#[must_use]
pub fn find_step_with_metadata(keyword: StepKeyword, text: StepText<'_>) -> Option<&'static Step> {
    resolve_step(keyword, text).inspect(|step| {
        mark_used((step.keyword, step.pattern));
    })
}

/// Return registered steps that were never executed.
#[must_use]
pub fn unused_steps() -> Vec<&'static Step> {
    let used = USED_STEPS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    all_steps()
        .into_iter()
        .filter(|s| !used.contains(&(s.keyword, s.pattern)))
        .collect()
}

/// Group step definitions that share a keyword and pattern.
#[must_use]
pub fn duplicate_steps() -> Vec<Vec<&'static Step>> {
    let mut groups: HashMap<StepKey, Vec<&'static Step>> = HashMap::new();
    for step in all_steps() {
        groups
            .entry((step.keyword, step.pattern))
            .or_default()
            .push(step);
    }
    groups.into_values().filter(|v| v.len() > 1).collect()
}

/// Serialize the registry to a JSON array.
///
/// Each entry records the step keyword, pattern, source location, and whether
/// the step has been executed. The JSON is intended for consumption by
/// diagnostic tooling such as `cargo bdd`.
///
/// # Errors
///
/// Returns an error if serialization fails.
///
/// # Examples
///
/// ```
/// use rstest_bdd::dump_registry;
///
/// let json = dump_registry().expect("serialize registry");
/// assert!(json.contains("\"steps\""));
/// ```
#[cfg(feature = "diagnostics")]
pub fn dump_registry() -> serde_json::Result<String> {
    diagnostics::dump_registry()
}
