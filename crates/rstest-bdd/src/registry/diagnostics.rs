//! Diagnostics-only registry exports.
//!
//! This module owns the data structures used to record bypassed steps and to
//! dump the registry for consumption by external tooling such as `cargo bdd`.
//! Keeping the implementation here keeps the core registry surface small and
//! helps keep `registry.rs` under the project file size limit.

use std::sync::{LazyLock, Mutex};

use hashbrown::HashSet;
use serde::Serialize;

use super::{
    StepKey,
    USED_STEPS,
    all_steps,
    bypassed::BypassedScenario,
    library_for_step,
    resolve_step,
    step_by_key,
};
use crate::{
    reporting::{self, ScenarioStatus},
    types::StepKeyword,
};

/// Recorded metadata for a bypassed step.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct BypassedStepRecord {
    /// Registry key for the bypassed step.
    pub(super) key: StepKey,
    /// Feature path containing the scenario.
    pub(super) feature_path: String,
    /// Name of the bypassed scenario.
    pub(super) scenario_name: String,
    /// Source line of the bypassed scenario.
    pub(super) scenario_line: u32,
    /// Tags attached to the scenario.
    pub(super) tags: Vec<String>,
    /// Closed library identities selected by the bypassed scenario.
    pub(super) libraries: Vec<&'static str>,
    /// Optional reason recorded for the bypass.
    pub(super) reason: Option<String>,
}

/// Process-wide registry of bypassed steps.
static BYPASSED_STEPS: LazyLock<Mutex<HashSet<BypassedStepRecord>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

/// Add a bypassed step record to the registry.
fn mark_bypassed(record: BypassedStepRecord) {
    BYPASSED_STEPS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(record);
}

/// Snapshot all currently recorded bypassed steps.
fn bypassed_records() -> Vec<BypassedStepRecord> {
    BYPASSED_STEPS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .cloned()
        .collect()
}

/// Record each registered step skipped by a scenario.
pub(super) fn record_bypassed_steps_impl<'a, I>(scenario: BypassedScenario<'_>, steps: I)
where
    I: IntoIterator<Item = (StepKeyword, &'a str)>,
{
    for (keyword, text) in steps {
        if let Ok(Some(step)) = resolve_step(scenario.scope, keyword, text.into()) {
            let record = BypassedStepRecord {
                key: (library_for_step(step), step.keyword, step.pattern.as_str()),
                feature_path: scenario.feature_path.to_owned(),
                scenario_name: scenario.scenario_name.to_owned(),
                scenario_line: scenario.scenario_line,
                tags: scenario.tags.to_owned(),
                libraries: scenario
                    .scope
                    .libraries()
                    .iter()
                    .map(|library| library.as_str())
                    .collect(),
                reason: scenario.reason.map(str::to_owned),
            };
            mark_bypassed(record);
        }
    }
}

/// Serializable registry step entry.
#[derive(Serialize)]
struct DumpedStep {
    /// Stable library identity that owns the step.
    library: &'static str,
    /// Step keyword.
    keyword: &'static str,
    /// Step pattern.
    pattern: &'static str,
    /// Source file defining the step.
    file: &'static str,
    /// Source line defining the step.
    line: u32,
    /// Whether the step was used.
    used: bool,
    /// Whether the step was bypassed.
    bypassed: bool,
}

/// Serializable scenario outcome entry.
#[derive(Serialize)]
struct DumpedScenario {
    /// Feature path containing the scenario.
    feature_path: String,
    /// Human-readable scenario name.
    scenario_name: String,
    /// Lowercase scenario status label.
    status: &'static str,
    /// Optional skip message.
    message: Option<String>,
    /// Whether skipping was explicitly allowed.
    allow_skipped: bool,
    /// Whether skipping forced a failure.
    forced_failure: bool,
    /// Source line of the scenario.
    line: u32,
    /// Scenario tags.
    tags: Vec<String>,
    /// Closed library identities selected by the scenario.
    libraries: Vec<&'static str>,
}

/// Serializable bypassed-step entry.
#[derive(Serialize)]
struct DumpedBypassedStep {
    /// Stable library identity that owns the bypassed step.
    library: &'static str,
    /// Step keyword.
    keyword: &'static str,
    /// Step pattern.
    pattern: &'static str,
    /// Source file defining the step.
    file: &'static str,
    /// Source line defining the step.
    line: u32,
    /// Feature path containing the scenario.
    feature_path: String,
    /// Scenario name.
    scenario_name: String,
    /// Source line of the scenario.
    scenario_line: u32,
    /// Scenario tags.
    tags: Vec<String>,
    /// Closed library identities selected by the bypassed scenario.
    libraries: Vec<&'static str>,
    /// Optional bypass reason.
    reason: Option<String>,
}

/// Top-level registry dump consumed by diagnostics tooling.
#[derive(Serialize)]
struct RegistryDump {
    /// Registered step entries.
    steps: Vec<DumpedStep>,
    /// Recorded scenario entries.
    scenarios: Vec<DumpedScenario>,
    /// Recorded bypassed-step entries.
    bypassed_steps: Vec<DumpedBypassedStep>,
}

/// Serialize the current step and scenario registry.
pub(super) fn dump_registry() -> serde_json::Result<String> {
    let used = USED_STEPS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let bypassed = bypassed_records();
    let bypassed_keys: HashSet<StepKey> = bypassed.iter().map(|entry| entry.key).collect();
    let steps: Vec<_> = all_steps()
        .into_iter()
        .map(|s| DumpedStep {
            library: library_for_step(s).as_str(),
            keyword: s.keyword.as_str(),
            pattern: s.pattern.as_str(),
            file: s.file,
            line: s.line,
            used: used.contains(&(library_for_step(s), s.keyword, s.pattern.as_str())),
            bypassed: bypassed_keys.contains(&(library_for_step(s), s.keyword, s.pattern.as_str())),
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
                libraries: record
                    .scope()
                    .libraries()
                    .iter()
                    .map(|library| library.as_str())
                    .collect(),
            }
        })
        .collect();

    let bypassed_steps = bypassed
        .into_iter()
        .filter_map(|entry| {
            step_by_key(entry.key).map(|step| DumpedBypassedStep {
                library: entry.key.0.as_str(),
                keyword: step.keyword.as_str(),
                pattern: step.pattern.as_str(),
                file: step.file,
                line: step.line,
                feature_path: entry.feature_path,
                scenario_name: entry.scenario_name,
                scenario_line: entry.scenario_line,
                tags: entry.tags,
                libraries: entry.libraries,
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
