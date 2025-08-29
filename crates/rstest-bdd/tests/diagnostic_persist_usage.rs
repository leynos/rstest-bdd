//! Cross-process step usage persistence.

use rstest_bdd::{StepContext, StepError, StepKeyword, step, unused_steps};
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

#[expect(
    clippy::unnecessary_wraps,
    reason = "wrapper must match StepFn signature"
)]
fn dummy(
    ctx: &StepContext<'_>,
    _text: &str,
    _doc: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<(), StepError> {
    let _ = ctx;
    Ok(())
}

step!(StepKeyword::Given, "a persisted step", dummy, &[]);

fn usage_file() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|e| panic!("resolve current executable: {e}"));
    exe.ancestors()
        .find(|p| p.file_name().is_some_and(|n| n == "target"))
        .unwrap_or_else(|| panic!("binary must live under target directory"))
        .join(".rstest-bdd-usage.json")
}

#[test]
fn persists_usage_to_file() {
    let path = usage_file();
    let _ = fs::remove_file(&path);

    let runner = rstest_bdd::find_step(StepKeyword::Given, "a persisted step".into())
        .unwrap_or_else(|| panic!("step not found"));
    runner(&StepContext::default(), "a persisted step", None, None)
        .unwrap_or_else(|e| panic!("run step: {e}"));

    let content = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read usage file: {e}"));
    assert!(content.contains("a persisted step"));

    let _ = fs::remove_file(&path);
}

#[test]
fn reads_persisted_usage() {
    let path = usage_file();
    let _ = fs::remove_file(&path);

    let record = json!({ "keyword": "Given", "pattern": "a persisted step" });
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap_or_else(|e| panic!("open usage file: {e}"));
    writeln!(file, "{record}").unwrap_or_else(|e| panic!("write usage: {e}"));

    let patterns: Vec<_> = unused_steps().iter().map(|s| s.pattern.as_str()).collect();
    assert!(!patterns.contains(&"a persisted step"));

    let _ = fs::remove_file(path);
}
