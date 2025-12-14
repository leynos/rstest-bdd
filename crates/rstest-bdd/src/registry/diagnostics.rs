//! Diagnostics-only registry exports.
//!
//! This module owns the data structures used to record bypassed steps and to
//! dump the registry for consumption by external tooling such as `cargo bdd`.
//! Keeping the implementation here keeps the core registry surface small and
//! helps keep `registry.rs` under the project file size limit.

use super::{StepKey, USED_STEPS, all_steps, resolve_step, step_by_key};
use crate::reporting::{self, ScenarioStatus};
use crate::types::StepKeyword;
use hashbrown::HashSet;
use serde::Serialize;
use std::sync::{LazyLock, Mutex};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct BypassedStepRecord {
    pub(super) key: StepKey,
    pub(super) feature_path: String,
    pub(super) scenario_name: String,
    pub(super) scenario_line: u32,
    pub(super) tags: Vec<String>,
    pub(super) reason: Option<String>,
}

static BYPASSED_STEPS: LazyLock<Mutex<HashSet<BypassedStepRecord>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

fn mark_bypassed(record: BypassedStepRecord) {
    BYPASSED_STEPS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(record);
}

fn bypassed_records() -> Vec<BypassedStepRecord> {
    BYPASSED_STEPS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .cloned()
        .collect()
}

pub(super) fn record_bypassed_steps_impl<'a, I>(
    feature_path: &str,
    scenario_name: &str,
    scenario_line: u32,
    tags: &[String],
    reason: Option<&str>,
    steps: I,
) where
    I: IntoIterator<Item = (StepKeyword, &'a str)>,
{
    for (keyword, text) in steps {
        if let Some(step) = resolve_step(keyword, text.into()) {
            let record = BypassedStepRecord {
                key: (step.keyword, step.pattern),
                feature_path: feature_path.to_string(),
                scenario_name: scenario_name.to_string(),
                scenario_line,
                tags: tags.to_owned(),
                reason: reason.map(str::to_owned),
            };
            mark_bypassed(record);
        }
    }
}

#[derive(Serialize)]
struct DumpedStep {
    keyword: &'static str,
    pattern: &'static str,
    file: &'static str,
    line: u32,
    used: bool,
    bypassed: bool,
}

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

#[derive(Serialize)]
struct RegistryDump {
    steps: Vec<DumpedStep>,
    scenarios: Vec<DumpedScenario>,
    bypassed_steps: Vec<DumpedBypassedStep>,
}

pub(super) fn dump_registry() -> serde_json::Result<String> {
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
