//! Shared fixtures for the nested-Cargo helper tests.
//!
//! The helpers are fallible so the calling test bodies assert:
//! `allow-expect-in-tests` covers a `#[cfg(test)]` module or a `#[test]`
//! body, not a plain `fn` in an integration-test crate.

use std::{ffi::OsString, path::PathBuf, process::Command};

use rstest_bdd_harness::nested_cargo::{NestedCargoEnv, ProfileDestination, env_from_vars};

/// Pair each key with its value as the nested helper expects them.
pub(super) fn vars(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    pairs
        .iter()
        .map(|(key, value)| (OsString::from(key), OsString::from(value)))
        .collect()
}

pub(super) fn fallback() -> PathBuf { PathBuf::from("/tmp/fallback-target") }

/// Build an environment for a process whose coverage the caller keeps.
pub(super) fn caller_env(vars: Vec<(OsString, OsString)>) -> std::io::Result<NestedCargoEnv> {
    env_from_vars(vars, Some(&fallback()), ProfileDestination::Caller)
}

/// Build an environment for a nested build whose coverage is redirected.
pub(super) fn scratch_env(vars: Vec<(OsString, OsString)>) -> std::io::Result<NestedCargoEnv> {
    env_from_vars(vars, Some(&fallback()), ProfileDestination::ChildScratch)
}

/// Look up one environment entry on a built command.
pub(super) fn cmd_env(cmd: &Command, key: &str) -> Option<OsString> {
    cmd.get_envs()
        .find(|(name, _)| name.to_string_lossy() == key)
        .and_then(|(_, value)| value.map(OsString::from))
}
