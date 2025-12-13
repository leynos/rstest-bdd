//! Step registration and lookup.
//! This module defines the `Step` record, the `step!` macro for registration,
//! and the global registry used to find steps by keyword and pattern or by
//! placeholder matching.

use crate::pattern::StepPattern;
use crate::placeholder::extract_placeholders;
#[cfg(feature = "diagnostics")]
use crate::reporting::{self, ScenarioStatus};
use crate::types::{PatternStr, StepFn, StepKeyword, StepText};
use hashbrown::{HashMap, HashSet};
use inventory::iter;
#[cfg(feature = "diagnostics")]
use serde::Serialize;
use std::hash::{BuildHasher, Hash, Hasher};
use std::sync::{LazyLock, Mutex};

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
        assert!(
            !map.contains_key(&key),
            "duplicate step for '{}' + '{}' defined at {}:{}",
            step.keyword.as_str(),
            step.pattern.as_str(),
            step.file,
            step.line
        );
        map.insert(key, step.run);
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

#[cfg(feature = "diagnostics")]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct BypassedStepRecord {
    key: StepKey,
    feature_path: String,
    scenario_name: String,
    scenario_line: u32,
    tags: Vec<String>,
    reason: Option<String>,
}

#[cfg(feature = "diagnostics")]
static BYPASSED_STEPS: LazyLock<Mutex<HashSet<BypassedStepRecord>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

#[cfg(feature = "diagnostics")]
fn mark_bypassed(record: BypassedStepRecord) {
    BYPASSED_STEPS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(record);
}

#[cfg(feature = "diagnostics")]
fn bypassed_records() -> Vec<BypassedStepRecord> {
    BYPASSED_STEPS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .cloned()
        .collect()
}

fn all_steps() -> Vec<&'static Step> {
    iter::<Step>.into_iter().collect()
}

fn step_by_key(key: StepKey) -> Option<&'static Step> {
    iter::<Step>
        .into_iter()
        .find(|step| (step.keyword, step.pattern) == key)
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
        .and_then(|(key, _)| step_by_key(*key))
}

fn resolve_step(keyword: StepKeyword, text: StepText<'_>) -> Option<&'static Step> {
    resolve_exact_step(keyword, text.as_str().into()).or_else(|| {
        iter::<Step>.into_iter().find(|step| {
            step.keyword == keyword && extract_placeholders(step.pattern, text).is_ok()
        })
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

/// Record step definitions that were bypassed after a scenario requested a skip.
#[cfg(feature = "diagnostics")]
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
    let feature_path = feature_path.into();
    let scenario_name = scenario_name.into();
    let tags = tags.into();
    let reason = reason.map(str::to_owned);

    for (keyword, text) in steps {
        if let Some(step) = resolve_step(keyword, text.into()) {
            let record = BypassedStepRecord {
                key: (step.keyword, step.pattern),
                feature_path: feature_path.clone(),
                scenario_name: scenario_name.clone(),
                scenario_line,
                tags: tags.clone(),
                reason: reason.clone(),
            };
            mark_bypassed(record);
        }
    }
}

#[cfg(feature = "diagnostics")]
#[derive(Serialize)]
struct DumpedStep {
    keyword: &'static str,
    pattern: &'static str,
    file: &'static str,
    line: u32,
    used: bool,
    #[serde(default)]
    bypassed: bool,
}

#[cfg(feature = "diagnostics")]
#[derive(Serialize)]
struct DumpedScenario {
    feature_path: String,
    scenario_name: String,
    status: &'static str,
    message: Option<String>,
    allow_skipped: bool,
    forced_failure: bool,
    line: u32,
    tags: Vec<String>,
}

#[cfg(feature = "diagnostics")]
#[derive(Serialize)]
struct DumpedBypassedStep {
    keyword: &'static str,
    pattern: &'static str,
    file: &'static str,
    line: u32,
    feature_path: String,
    scenario_name: String,
    scenario_line: u32,
    tags: Vec<String>,
    reason: Option<String>,
}

#[cfg(feature = "diagnostics")]
#[derive(Serialize)]
struct RegistryDump {
    steps: Vec<DumpedStep>,
    scenarios: Vec<DumpedScenario>,
    #[serde(default)]
    bypassed_steps: Vec<DumpedBypassedStep>,
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
    let used = USED_STEPS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let bypassed = bypassed_records();
    let bypassed_keys: HashSet<StepKey> = bypassed.iter().map(|entry| entry.key).collect();
    let steps: Vec<_> = all_steps()
        .into_iter()
        .map(|s| DumpedStep {
            keyword: s.keyword.as_str(),
            pattern: s.pattern.as_str(),
            file: s.file,
            line: s.line,
            used: used.contains(&(s.keyword, s.pattern)),
            bypassed: bypassed_keys.contains(&(s.keyword, s.pattern)),
        })
        .collect();

    let scenarios = reporting::snapshot()
        .into_iter()
        .map(|record| {
            let (status, message, allow_skipped, forced_failure) = match record.status() {
                ScenarioStatus::Passed => ("passed", None, false, false),
                ScenarioStatus::Skipped(details) => (
                    "skipped",
                    details.message().map(str::to_owned),
                    details.allow_skipped(),
                    details.forced_failure(),
                ),
            };
            DumpedScenario {
                feature_path: record.feature_path().to_owned(),
                scenario_name: record.scenario_name().to_owned(),
                status,
                message,
                allow_skipped,
                forced_failure,
                line: record.line(),
                tags: record.tags().to_vec(),
            }
        })
        .collect();

    let bypassed_steps = bypassed
        .into_iter()
        .filter_map(|entry| {
            step_by_key(entry.key).map(|step| DumpedBypassedStep {
                keyword: step.keyword.as_str(),
                pattern: step.pattern.as_str(),
                file: step.file,
                line: step.line,
                feature_path: entry.feature_path,
                scenario_name: entry.scenario_name,
                scenario_line: entry.scenario_line,
                tags: entry.tags,
                reason: entry.reason,
            })
        })
        .collect();

    serde_json::to_string(&RegistryDump {
        steps,
        scenarios,
        bypassed_steps,
    })
}
