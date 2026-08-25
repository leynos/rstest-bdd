//! JSON writer for scenario outcome records.
//!
//! The writer serializes a snapshot of the scenario collector into a
//! predictable, machine-readable shape. The schema keeps status labels in
//! lowercase so downstream tools can rely on consistent casing.

use std::io::Write;

use serde::Serialize;

use super::{ScenarioRecord, ScenarioStatus, snapshot};

/// Top-level JSON report payload.
#[derive(Serialize)]
struct JsonReport<'a> {
    /// Serialized scenario records.
    scenarios: Vec<JsonScenario<'a>>,
}

/// JSON representation of one scenario outcome.
#[derive(Serialize)]
struct JsonScenario<'a> {
    /// Feature path containing the scenario.
    feature_path: &'a str,
    /// Human-readable scenario name.
    scenario_name: &'a str,
    /// Lowercase scenario status label.
    status: &'static str,
    /// Source line where the scenario is declared.
    line: u32,
    /// Scenario tags.
    tags: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Details recorded when the scenario was skipped.
    skip: Option<JsonSkip<'a>>,
}

/// JSON representation of a skipped scenario.
#[derive(Serialize)]
struct JsonSkip<'a> {
    /// Optional skip message.
    message: Option<&'a str>,
    /// Whether skipping was explicitly allowed.
    allow_skipped: bool,
    /// Whether skipping forced a failure.
    forced_failure: bool,
}

impl<'a> From<&'a [ScenarioRecord]> for JsonReport<'a> {
    fn from(records: &'a [ScenarioRecord]) -> Self {
        let scenarios = records.iter().map(JsonScenario::from).collect();
        Self { scenarios }
    }
}

impl<'a> From<&'a ScenarioRecord> for JsonScenario<'a> {
    fn from(record: &'a ScenarioRecord) -> Self {
        let skip = match record.status() {
            ScenarioStatus::Passed => None,
            ScenarioStatus::Skipped(details) => Some(JsonSkip {
                message: details.message(),
                allow_skipped: details.allow_skipped(),
                forced_failure: details.forced_failure(),
            }),
        };
        Self {
            feature_path: record.feature_path(),
            scenario_name: record.scenario_name(),
            status: record.status().label(),
            line: record.line(),
            tags: record.tags(),
            skip,
        }
    }
}

/// Serialize the provided scenario records into the supplied writer.
///
/// # Examples
/// ```rust
/// use rstest_bdd::reporting::{ScenarioMetadata, ScenarioRecord, ScenarioStatus, json};
///
/// let metadata = ScenarioMetadata::new("feature", "scenario", 1, Vec::new());
/// let records = vec![ScenarioRecord::from_metadata(
///     metadata,
///     ScenarioStatus::Passed,
/// )];
/// let mut buffer = Vec::new();
/// json::write(&mut buffer, &records).unwrap();
/// let output = String::from_utf8(buffer).unwrap();
/// assert!(output.contains("\"status\":\"passed\""));
/// ```
///
/// # Errors
/// Returns an error when serialization of the provided records fails.
pub fn write<W: Write>(writer: &mut W, records: &[ScenarioRecord]) -> serde_json::Result<()> {
    serde_json::to_writer(writer, &JsonReport::from(records))
}

/// Serialize the current collector snapshot into the supplied writer.
///
/// # Examples
/// ```rust
/// use rstest_bdd::reporting::{ScenarioMetadata, ScenarioRecord, ScenarioStatus, json, record};
///
/// let metadata = ScenarioMetadata::new("feature", "scenario", 1, Vec::new());
/// record(ScenarioRecord::from_metadata(
///     metadata,
///     ScenarioStatus::Passed,
/// ));
/// let mut buffer = Vec::new();
/// json::write_snapshot(&mut buffer).unwrap();
/// assert!(!buffer.is_empty());
/// ```
///
/// # Errors
/// Returns an error when serializing the snapshot fails.
pub fn write_snapshot<W: Write>(writer: &mut W) -> serde_json::Result<()> {
    let snapshot = snapshot();
    write(writer, &snapshot)
}

/// Produce a JSON string representation of the provided scenario records.
///
/// # Examples
/// ```rust
/// use rstest_bdd::reporting::{ScenarioMetadata, ScenarioRecord, ScenarioStatus, json};
///
/// let metadata = ScenarioMetadata::new("feature", "scenario", 1, Vec::new());
/// let records = vec![ScenarioRecord::from_metadata(
///     metadata,
///     ScenarioStatus::Passed,
/// )];
/// let json = json::to_string(&records).unwrap();
/// assert!(json.contains("\"scenario_name\":\"scenario\""));
/// ```
///
/// # Errors
/// Returns an error when serializing the provided records fails.
pub fn to_string(records: &[ScenarioRecord]) -> serde_json::Result<String> {
    serde_json::to_string(&JsonReport::from(records))
}

/// Produce a JSON string representation of the current collector snapshot.
///
/// # Examples
/// ```rust
/// use rstest_bdd::reporting::{ScenarioMetadata, ScenarioRecord, ScenarioStatus, json, record};
///
/// let metadata = ScenarioMetadata::new("feature", "scenario", 1, Vec::new());
/// record(ScenarioRecord::from_metadata(
///     metadata,
///     ScenarioStatus::Passed,
/// ));
/// let json = json::snapshot_string().unwrap();
/// assert!(json.contains("\"feature_path\":\"feature\""));
/// ```
///
/// # Errors
/// Returns an error when serializing the snapshot fails.
pub fn snapshot_string() -> serde_json::Result<String> {
    let snapshot = snapshot();
    to_string(&snapshot)
}
