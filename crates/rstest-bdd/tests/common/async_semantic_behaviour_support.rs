//! Shared support for async semantic behaviour tests.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard};

#[cfg(feature = "diagnostics")]
use serde_json::Value;

pub(crate) const FEATURE_PATH: &str = "tests/features/async_semantic_behaviour.feature";
pub(crate) const SKIP_SCENARIO_NAME: &str = "async skip propagation preserves metadata";
pub(crate) const SKIP_SCENARIO_LINE: u32 = 4;
pub(crate) const ERROR_SCENARIO_NAME: &str = "async failure surfaces scenario metadata";

static EVENTS: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));
static CLEANUP_DROPS: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SemanticValue(pub(crate) i32);

pub(crate) struct CleanupProbe;

impl Drop for CleanupProbe {
    fn drop(&mut self) {
        CLEANUP_DROPS.fetch_add(1, Ordering::SeqCst);
    }
}

fn events_guard() -> MutexGuard<'static, Vec<String>> {
    match EVENTS.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub(crate) fn clear_events() {
    events_guard().clear();
}

pub(crate) fn push_event(event: impl Into<String>) {
    events_guard().push(event.into());
}

pub(crate) fn snapshot_events() -> Vec<String> {
    events_guard().clone()
}

pub(crate) fn reset_cleanup_drops() {
    CLEANUP_DROPS.store(0, Ordering::SeqCst);
}

pub(crate) fn cleanup_drops() -> usize {
    CLEANUP_DROPS.load(Ordering::SeqCst)
}

pub(crate) fn assert_feature_path_suffix(actual: &str, expected_suffix: &str) {
    let actual_path = Path::new(actual);
    let expected = Path::new(expected_suffix);
    assert!(
        actual_path.ends_with(expected),
        "feature path should reference {expected_suffix}, got {actual}",
    );
}

pub(crate) fn assert_message_mentions_feature_path(message: &str, expected_suffix: &str) {
    let normalized_message = message.replace('\\', "/");
    assert!(
        normalized_message.contains(expected_suffix),
        "panic message should include the feature path {expected_suffix}: {message}",
    );
}

#[cfg(feature = "diagnostics")]
pub(crate) fn assert_bypassed_step_recorded(
    scenario_name: &str,
    scenario_line: u32,
    step_pattern: &str,
    reason: &str,
) {
    let dump = match rstest_bdd::dump_registry() {
        Ok(dump) => dump,
        Err(error) => panic!("registry dump should serialize: {error}"),
    };
    let parsed: Value = match serde_json::from_str(&dump) {
        Ok(parsed) => parsed,
        Err(error) => panic!("registry dump should be valid JSON: {error}"),
    };
    let bypassed_steps = parsed
        .get("bypassed_steps")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("registry dump should include bypassed_steps"));
    assert!(
        bypassed_steps.iter().any(|entry| {
            entry.get("scenario_name") == Some(&Value::String(scenario_name.into()))
                && entry.get("scenario_line").and_then(Value::as_u64)
                    == Some(u64::from(scenario_line))
                && entry.get("pattern") == Some(&Value::String(step_pattern.into()))
                && entry
                    .get("reason")
                    .and_then(Value::as_str)
                    .is_some_and(|message| message.contains(reason))
        }),
        "expected a bypassed-step record for scenario '{scenario_name}' and pattern '{step_pattern}'",
    );
}
