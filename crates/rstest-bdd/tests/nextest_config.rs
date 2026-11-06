//! Integration contract for the workspace's nextest override and timeout budget
//! policy.
//!
//! The suite reads `.config/nextest.toml` as data rather than running
//! cargo-nextest: `nextest_configuration` parses the file with `toml`, and the
//! tests assert the budget policy a reviewer can see written down there. The
//! run-level ceiling and the test group's thread limit come from the same
//! parsed value, read through `default_profile_key` and
//! `test_group_max_threads` respectively.
//!
//! Four contracts are pinned:
//!
//! * `preparation_nextest_override_preserves_timeout_contract` requires exactly one override to
//!   match `test(/nested_fixture_preparation$/)`, at a 30 m period with the same termination
//!   settings and group, and to be scheduled ahead of every other override (`priority` 100 against
//!   the default 0). It also requires the top-level `nextest-version` that `priority` needs, and
//!   that the preparation override is the first one in the file, so the first-match precedence rule
//!   keeps it authoritative if a future override ever matched the same tests.
//! * `trybuild_nextest_override_preserves_timeout_contract` requires exactly one override to name
//!   any of the four compile-test binaries, and that override's filter to name all four and no
//!   others. It then requires the override to keep a 20 m `slow-timeout` period, `terminate-after =
//!   1`, a 5 s grace period and membership of the `cargo-spawning` group, and the default profile's
//!   `global-timeout` to exceed the period in seconds.
//! * `feature_rebuild_nextest_override_preserves_timeout_contract` requires exactly one override to
//!   carry the exact filter `binary_id(rstest-bdd::feature_rebuild_invalidation)`, with the same
//!   termination settings and group, at a 600 s period.
//! * `nested_cargo_run_retains_cold_build_reserve` recomputes the R-6 sizing scenario (a reference
//!   lane of `180 + 600 * 4 + 600 * 3` seconds, ten cold nested invocations at 19 s each and a 600
//!   s reserve) and requires the default profile's `global-timeout` to cover it, and the
//!   `cargo-spawning` group's `max-threads` to stay at 1 so nested builds keep serializing.
//!
//! The three timed overrides are asserted at their exact periods rather than only
//! against the global timeout, because widening one is how a cold build gets
//! hidden instead of moved: the preparation override is the only place in the
//! file where a cold dependency build may spend a test's budget.
//!
//! The overrides, the group and the run-level ceiling are policy that a future
//! edit could break silently, so these assertions fail loudly instead. They
//! cover the two nextest tiers of the four-tier timeout budget: a per-test
//! `slow-timeout` compared against the whole-run `global-timeout`. The
//! coverage watchdog and the job `timeout-minutes` are the outer two tiers,
//! covered by the workflow contracts under `tests/workflow_contracts`;
//! `.config/nextest.toml` asks that suite and this file to be kept in step.
//!
//! Reading the parsed value is `support`'s job, attached with
//! `#[path = "nextest_config/support.rs"]` so Cargo does not also compile it as
//! a second test target. This module states the policy; that one finds the keys
//! it is stated about.

use std::{error::Error, io};

use rstest_bdd_harness::nested_cargo;
use toml::Value;

#[path = "nextest_config/support.rs"]
mod support;
use support::{
    binary_id_report,
    default_global_timeout,
    default_overrides,
    duration_seconds,
    nextest_configuration,
    nextest_version,
    test_group_max_threads,
    trybuild_override_candidates,
};

const FEATURE_REBUILD_BINARY: &str = "rstest-bdd::feature_rebuild_invalidation";

/// The filter every preparation override must carry.
const PREPARATION_FILTER: &str = "test(/nested_fixture_preparation$/)";

#[test]
fn preparation_nextest_override_preserves_timeout_contract() -> Result<(), Box<dyn Error>> {
    let configuration = nextest_configuration()?;
    let overrides = default_overrides(&configuration)?;
    let override_value = single_preparation_override(overrides)?;
    assert_preparation_budget(override_value)?;
    assert_preparation_scheduling(&configuration, overrides, override_value)?;
    Ok(())
}

/// The one override that matches the preparation tests, asserting both its
/// count and its position in the file.
///
/// nextest applies the first override that matches a test and configures a
/// setting, so both are part of the contract: a second matching override would
/// be dead policy, and a later position would silently defer to an earlier
/// match.
fn single_preparation_override(overrides: &[Value]) -> Result<&Value, Box<dyn Error>> {
    let candidates = overrides
        .iter()
        .enumerate()
        .filter(|(_, override_value)| {
            override_value.get("filter").and_then(Value::as_str) == Some(PREPARATION_FILTER)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        candidates.len(),
        1,
        "the preparation tests must have exactly one nextest override matching \
         {PREPARATION_FILTER}, found {}",
        candidates.len(),
    );
    let (found_position, override_value) = candidates
        .first()
        .copied()
        .ok_or_else(|| io::Error::other("expected one preparation nextest override"))?;
    assert_eq!(
        found_position, 0,
        "the preparation override must be first in the file; nextest applies the first override \
         that matches a test and configures a setting, so a later position would silently defer \
         to an earlier match",
    );
    Ok(override_value)
}

/// The preparation override's time budget and serialization group.
///
/// The 30-minute period is where a cold dependency build belongs. A timed
/// assertion has no budget for one, so this is the only override in the file
/// whose period may be sized for a build rather than an assertion.
fn assert_preparation_budget(override_value: &Value) -> Result<(), Box<dyn Error>> {
    let slow_timeout = override_value
        .get("slow-timeout")
        .and_then(Value::as_table)
        .ok_or_else(|| {
            io::Error::other("the preparation override must contain a slow-timeout table")
        })?;
    assert_eq!(
        slow_timeout.get("period").and_then(Value::as_str),
        Some("30m"),
        "the preparation override must allow a 30-minute period, which is where a cold dependency \
         build belongs; a timed assertion has no budget for one",
    );
    assert_eq!(
        slow_timeout
            .get("terminate-after")
            .and_then(Value::as_integer),
        Some(1),
        "the preparation override must terminate after one slow-timeout period",
    );
    assert_eq!(
        slow_timeout.get("grace-period").and_then(Value::as_str),
        Some("5s"),
        "the preparation override must retain the five-second grace period",
    );
    assert_eq!(
        override_value.get("test-group").and_then(Value::as_str),
        Some("cargo-spawning"),
        "the preparation override must remain in the cargo-spawning group, so it serializes with \
         the tests whose build trees it warms",
    );
    Ok(())
}

/// The preparation override's scheduling priority, and the runner version that
/// makes `priority` meaningful.
///
/// Raising priority is only safe because no other override competes for the
/// same value: nextest documents `priority` as ordering within a test group,
/// so a rival override at the same priority would make the preparation's
/// precedence over the assertions it warms accidental.
fn assert_preparation_scheduling(
    configuration: &Value,
    overrides: &[Value],
    override_value: &Value,
) -> Result<(), Box<dyn Error>> {
    let priority = override_value.get("priority").and_then(Value::as_integer);
    assert_eq!(
        priority,
        Some(100),
        "the preparation override must carry the highest scheduling priority nextest accepts, so \
         the preparations run before the timed assertions that depend on them",
    );
    let competing = overrides
        .iter()
        .filter(|other| {
            !std::ptr::eq(*other, override_value)
                && other
                    .get("priority")
                    .and_then(Value::as_integer)
                    .unwrap_or(0)
                    >= 100
        })
        .count();
    assert_eq!(
        competing, 0,
        "no other override may share the preparation override's scheduling priority",
    );

    let version = nextest_version(configuration)?;
    assert!(
        version >= (0, 9, 91),
        "nextest-version must be at least 0.9.91 so `priority` is a setting the runner reads, \
         found {version:?}",
    );
    Ok(())
}

#[test]
fn trybuild_nextest_override_preserves_timeout_contract() -> Result<(), Box<dyn Error>> {
    let configuration = nextest_configuration()?;
    let overrides = default_overrides(&configuration)?;
    let candidates = trybuild_override_candidates(overrides);
    let binary_id_report = binary_id_report(overrides, &candidates);

    assert_eq!(
        candidates.len(),
        1,
        "expected exactly one trybuild nextest override, found {}. {binary_id_report}",
        candidates.len(),
    );

    let Some(trybuild_override) = candidates.first().and_then(|index| overrides.get(*index)) else {
        return Err(io::Error::other("expected one matching trybuild nextest override").into());
    };
    assert_eq!(
        binary_id_report,
        "missing binary IDs: <none>; duplicate binary IDs: <none>; unexpected binary IDs: <none>",
        "trybuild nextest override must identify exactly the expected binaries. {binary_id_report}",
    );

    let slow_timeout = trybuild_override
        .get("slow-timeout")
        .and_then(Value::as_table)
        .expect("trybuild nextest override must contain a slow-timeout table");

    let period = slow_timeout
        .get("period")
        .and_then(Value::as_str)
        .ok_or_else(|| io::Error::other("trybuild nextest override must name a period"))?;
    assert_eq!(
        period, "20m",
        "trybuild nextest override must allow a 20-minute slow timeout, which is what a cold \
         compiler cache costs this fixture tree",
    );

    let global_timeout = default_global_timeout(&configuration)?;
    assert!(
        duration_seconds(global_timeout)? > duration_seconds(period)?,
        "the default profile's global timeout ({global_timeout}) must exceed the trybuild slow \
         timeout ({period}); otherwise nextest kills the run before the test the budget exists \
         for can finish, and raising the budget alone rescues nothing",
    );
    assert_eq!(
        slow_timeout
            .get("terminate-after")
            .and_then(Value::as_integer),
        Some(1),
        "trybuild nextest override must terminate after one slow-timeout period",
    );
    assert_eq!(
        slow_timeout.get("grace-period").and_then(Value::as_str),
        Some("5s"),
        "trybuild nextest override must retain its five-second grace period",
    );
    assert_eq!(
        trybuild_override.get("test-group").and_then(Value::as_str),
        Some("cargo-spawning"),
        "trybuild nextest override must remain in the cargo-spawning group",
    );

    Ok(())
}

#[test]
fn feature_rebuild_nextest_override_preserves_timeout_contract() -> Result<(), Box<dyn Error>> {
    let configuration = nextest_configuration()?;
    let overrides = default_overrides(&configuration)?;
    let filter = format!("binary_id({FEATURE_REBUILD_BINARY})");
    let matching = overrides
        .iter()
        .filter(|override_value| {
            override_value.get("filter").and_then(Value::as_str) == Some(filter.as_str())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "the feature-rebuild test must have exactly one exact nextest override",
    );
    let override_value = matching
        .first()
        .expect("the asserted single feature-rebuild override must be present");
    let slow_timeout = override_value
        .get("slow-timeout")
        .and_then(Value::as_table)
        .expect("feature-rebuild override must contain a slow-timeout table");
    assert_eq!(
        slow_timeout.get("period").and_then(Value::as_str),
        Some("600s")
    );
    assert_eq!(
        slow_timeout
            .get("terminate-after")
            .and_then(Value::as_integer),
        Some(1)
    );
    assert_eq!(
        slow_timeout.get("grace-period").and_then(Value::as_str),
        Some("5s")
    );
    assert_eq!(
        override_value.get("test-group").and_then(Value::as_str),
        Some("cargo-spawning")
    );
    Ok(())
}

/// Keep cold-build drift from consuming the run-level reserve again.
/// The per-command preparation bound must fire before the runner's test bound.
///
/// A preparation test may run several sequential bounded commands: the
/// rebuild-invalidation preparation compiles one fixture, then runs the
/// addition fixture's metadata preflight and its compile. The contract is
/// `MAX_SEQUENTIAL_COMMANDS × PREPARATION_COMMAND_TIMEOUT < preparation period`,
/// recomputed from both constants rather than from a number repeated here, so
/// raising the command count without resizing the bound fails loudly. The
/// named timeout error this protects is the only report that carries the
/// child's captured output; a runner slow-timeout discards it.
#[test]
fn preparation_command_bound_fires_before_the_runner_kills_the_test() -> Result<(), Box<dyn Error>>
{
    let configuration = nextest_configuration()?;
    let overrides = default_overrides(&configuration)?;
    let preparation = single_preparation_override(overrides)?;
    let period = preparation
        .get("slow-timeout")
        .and_then(Value::as_table)
        .and_then(|slow_timeout| slow_timeout.get("period"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            io::Error::other("the preparation override must set a slow-timeout period")
        })?;
    let period_seconds = duration_seconds(period)?;
    let sequential_commands = nested_cargo::MAX_SEQUENTIAL_COMMANDS;
    let command_seconds = nested_cargo::PREPARATION_COMMAND_TIMEOUT.as_secs();
    assert!(
        command_seconds * sequential_commands < period_seconds,
        "the {command_seconds}s per-command preparation bound times {sequential_commands} \
         sequential commands must stay below the preparation override's {period_seconds}s period, \
         or nextest kills the test before the named timeout error can report it",
    );
    Ok(())
}

#[test]
fn nested_cargo_run_retains_cold_build_reserve() -> Result<(), Box<dyn Error>> {
    let configuration = nextest_configuration()?;
    // R-6 is a sizing scenario, not a sum of every test's maximum. Keep its
    // baseline and cold-build perturbation separate so neither is mistaken
    // for measured elapsed time on this host.
    let reference_lane_seconds = 180 + 600 * 4 + 600 * 3;
    let cold_invocations = 10;
    let additional_seconds_per_invocation = 19;
    let minimum_reserve_seconds = 600;
    let required = reference_lane_seconds
        + cold_invocations * additional_seconds_per_invocation
        + minimum_reserve_seconds;
    let actual = duration_seconds(default_global_timeout(&configuration)?)?;
    assert!(
        actual >= required,
        "nested Cargo needs {required}s including cold-build drift and reserve, got {actual}s",
    );
    assert_eq!(
        test_group_max_threads(&configuration, "cargo-spawning")?,
        1,
        "nested Cargo builds must remain serialized to avoid competing for compiler resources",
    );
    Ok(())
}
