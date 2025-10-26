//! Runtime configuration for rstest-bdd.
//!
//! The module currently exposes the `fail_on_skipped` flag controlling whether
//! skipped scenarios should panic when they lack the `@allow_skipped` tag.

use std::sync::atomic::{AtomicU8, Ordering};

const OVERRIDE_UNSET: u8 = 0;
const OVERRIDE_FALSE: u8 = 1;
const OVERRIDE_TRUE: u8 = 2;

static FAIL_ON_SKIPPED_OVERRIDE: AtomicU8 = AtomicU8::new(OVERRIDE_UNSET);

fn parse_env_bool(value: &str) -> Option<bool> {
    match value.trim() {
        "1" | "true" | "TRUE" | "True" | "yes" | "YES" | "Yes" | "on" | "ON" | "On" => Some(true),
        "0" | "false" | "FALSE" | "False" | "no" | "NO" | "No" | "off" | "OFF" | "Off" => {
            Some(false)
        }
        _ => None,
    }
}

fn env_fail_on_skipped() -> Option<bool> {
    std::env::var("RSTEST_BDD_FAIL_ON_SKIPPED")
        .ok()
        .as_deref()
        .and_then(parse_env_bool)
}

fn override_state() -> Option<bool> {
    match FAIL_ON_SKIPPED_OVERRIDE.load(Ordering::Relaxed) {
        OVERRIDE_FALSE => Some(false),
        OVERRIDE_TRUE => Some(true),
        _ => None,
    }
}

/// Determine whether skipped scenarios should panic.
#[must_use]
pub fn fail_on_skipped() -> bool {
    override_state()
        .or_else(env_fail_on_skipped)
        .unwrap_or(false)
}

/// Override the `fail_on_skipped` flag for the current process.
///
/// Tests may call [`clear_fail_on_skipped_override`] to restore environment
/// driven behaviour after toggling the override.
pub fn set_fail_on_skipped(enabled: bool) {
    let value = if enabled {
        OVERRIDE_TRUE
    } else {
        OVERRIDE_FALSE
    };
    FAIL_ON_SKIPPED_OVERRIDE.store(value, Ordering::Relaxed);
}

/// Remove any in-process override for the `fail_on_skipped` flag.
pub fn clear_fail_on_skipped_override() {
    FAIL_ON_SKIPPED_OVERRIDE.store(OVERRIDE_UNSET, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn reset_override() {
        clear_fail_on_skipped_override();
    }

    #[test]
    #[serial]
    fn default_is_false() {
        reset_override();
        assert!(!fail_on_skipped());
    }

    #[test]
    #[serial]
    fn override_sets_flag() {
        reset_override();
        set_fail_on_skipped(true);
        assert!(fail_on_skipped());
        set_fail_on_skipped(false);
        assert!(!fail_on_skipped());
        reset_override();
    }

    #[test]
    fn parse_env_bool_understands_common_values() {
        for truthy in [
            "1", "true", "TRUE", "True", "yes", "YES", "Yes", "on", "ON", "On",
        ] {
            assert_eq!(
                parse_env_bool(truthy),
                Some(true),
                "expected {truthy} to be truthy"
            );
        }
        for falsy in [
            "0", "false", "FALSE", "False", "no", "NO", "No", "off", "OFF", "Off",
        ] {
            assert_eq!(
                parse_env_bool(falsy),
                Some(false),
                "expected {falsy} to be falsy"
            );
        }
        assert_eq!(parse_env_bool("maybe"), None);
    }
}
