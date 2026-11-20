//! The timed override for the rebuild suite, and the run-level reserve.
//!
//! Split out of [`super`] so each file states one policy area. The parent
//! module owns the preparation and trybuild overrides; this one owns the two
//! contracts about the third timed override and the whole-run ceiling that
//! must cover it.
//!
//! Reading the parsed value is `support`'s job. This module states the policy;
//! that one finds the keys it is stated about.

use std::{error::Error, io};

use rstest_bdd_harness::nested_cargo;
use toml::Value;

use super::{
    FEATURE_REBUILD_BINARY,
    single_preparation_override,
    support::{
        default_global_timeout,
        default_overrides,
        duration_seconds,
        nextest_configuration,
        test_group_max_threads,
    },
};

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
