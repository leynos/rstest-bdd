//! Step registration and lookup.
//! This module defines the `Step` record, the `step!` macro for registration,
//! and the global registry used to find steps by keyword and pattern or by
//! placeholder matching.

use crate::pattern::StepPattern;
use crate::placeholder::extract_placeholders;
use crate::types::{PatternStr, StepFn, StepKeyword, StepText};
use hashbrown::{HashMap, HashSet};
use inventory::iter;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::hash::{BuildHasher, Hash, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::str::FromStr;
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

static USED_STEPS: LazyLock<Mutex<HashSet<StepKey>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

#[derive(Serialize, Deserialize, Eq, PartialEq, Hash)]
struct StepUsage {
    keyword: String,
    pattern: String,
}

fn usage_file_path() -> PathBuf {
    static PATH: LazyLock<PathBuf> = LazyLock::new(|| {
        let exe =
            std::env::current_exe().unwrap_or_else(|e| panic!("resolve current executable: {e}"));
        let target = exe
            .ancestors()
            .find(|p| p.file_name().is_some_and(|n| n == "target"))
            .unwrap_or_else(|| panic!("binary must live under target directory"));
        target.join(".rstest-bdd-usage.json")
    });
    PATH.clone()
}

fn mark_used(key: StepKey) {
    {
        let mut used = USED_STEPS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        used.insert(key);
    }

    let record = StepUsage {
        keyword: key.0.as_str().to_string(),
        pattern: key.1.as_str().to_string(),
    };
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(usage_file_path())
    {
        if serde_json::to_writer(&mut file, &record).is_ok() {
            let _ = writeln!(file);
        }
    }
}

fn read_used_from_file() -> HashSet<(StepKeyword, String)> {
    let mut used = HashSet::new();
    if let Ok(file) = fs::File::open(usage_file_path()) {
        let reader = BufReader::new(file);
        for line in reader.lines().map_while(Result::ok) {
            if let Ok(rec) = serde_json::from_str::<StepUsage>(&line) {
                if let Ok(keyword) = StepKeyword::from_str(&rec.keyword) {
                    used.insert((keyword, rec.pattern));
                }
            }
        }
    }
    used
}

fn combined_used() -> HashSet<(StepKeyword, String)> {
    let mut used = read_used_from_file();
    let mem = USED_STEPS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    used.extend(mem.iter().map(|(kw, pat)| (*kw, pat.as_str().to_string())));
    used
}

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
        .from_hash(hash, |(kw, pat)| {
            *kw == keyword && pat.as_str() == pattern.as_str()
        })
        .map(|(key, &f)| {
            mark_used(*key);
            f
        })
}

/// Find a registered step whose pattern matches the provided text.
#[must_use]
pub fn find_step(keyword: StepKeyword, text: StepText<'_>) -> Option<StepFn> {
    if let Some(f) = lookup_step(keyword, text.as_str().into()) {
        return Some(f);
    }
    for step in iter::<Step> {
        if step.keyword == keyword && extract_placeholders(step.pattern, text).is_ok() {
            mark_used((step.keyword, step.pattern));
            return Some(step.run);
        }
    }
    None
}

/// Return registered steps that were never executed.
#[must_use]
pub fn unused_steps() -> Vec<&'static Step> {
    let used = combined_used();
    iter::<Step>
        .into_iter()
        .filter(|s| {
            let key = (s.keyword, s.pattern.as_str().to_string());
            !used.contains(&key)
        })
        .collect()
}

/// Group step definitions that share a keyword and pattern.
#[must_use]
pub fn duplicate_steps() -> Vec<Vec<&'static Step>> {
    let mut groups: HashMap<StepKey, Vec<&'static Step>> = HashMap::new();
    for step in iter::<Step> {
        groups
            .entry((step.keyword, step.pattern))
            .or_default()
            .push(step);
    }
    groups.into_values().filter(|v| v.len() > 1).collect()
}

#[derive(Serialize)]
struct DumpedStep {
    keyword: &'static str,
    pattern: &'static str,
    file: &'static str,
    line: u32,
    used: bool,
}

/// Serialise the registry to a JSON array.
///
/// Each entry records the step keyword, pattern, source location, and whether
/// the step has been executed. The JSON is intended for consumption by
/// diagnostic tooling such as `cargo bdd`.
///
/// # Errors
///
/// Returns an error if serialisation fails.
///
/// # Examples
///
/// ```
/// use rstest_bdd::dump_registry;
///
/// let json = dump_registry().expect("serialise registry");
/// assert!(json.starts_with("["));
/// ```
pub fn dump_registry() -> serde_json::Result<String> {
    let used = combined_used();
    let steps: Vec<_> = iter::<Step>
        .into_iter()
        .map(|s| {
            let key = (s.keyword, s.pattern.as_str().to_string());
            DumpedStep {
                keyword: s.keyword.as_str(),
                pattern: s.pattern.as_str(),
                file: s.file,
                line: s.line,
                used: used.contains(&key),
            }
        })
        .collect();
    serde_json::to_string(&steps)
}
