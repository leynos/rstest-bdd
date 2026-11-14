//! The preparation override's own contracts.
//!
//! The nested-Cargo preparation is the only place in `.config/nextest.toml`
//! where a cold dependency build may spend a timed test's budget, so its
//! override carries four separate promises: it is the only override matching
//! the preparation tests, it is first in the file, it is scheduled ahead of the
//! assertions it warms, and its per-command bound still fires before the
//! runner's test bound. They are asserted together here because a change to
//! one is nearly always a change to the budget the others describe.
//!
//! # Why this is a separate module
//!
//! The parent module states the whole override policy; this one states the
//! preparation's share of it, which is large enough on its own to take the
//! parent past its file-length budget. The split is by cohesion: the
//! preparation override and its command bound are one subject, and the
//! filtered-override and trybuild contracts the parent keeps are another.

use std::{error::Error, io};

use rstest_bdd_harness::nested_cargo;
use toml::Value;

use super::support::{default_overrides, duration_seconds, nextest_configuration, nextest_version};

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
        concat!(
            "the preparation override must be first in the file; nextest applies the first \
             override ",
            "that matches a test and configures a setting, so a later position would silently \
             defer ",
            "to an earlier match",
        ),
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
        concat!(
            "the preparation override must allow a 30-minute period, which is where a cold \
             dependency ",
            "build belongs; a timed assertion has no budget for one",
        ),
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
        concat!(
            "the preparation override must remain in the cargo-spawning group, so it serializes \
             with ",
            "the tests whose build trees it warms",
        ),
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
        concat!(
            "the preparation override must carry the highest scheduling priority nextest accepts, \
             so ",
            "the preparations run before the timed assertions that depend on them",
        ),
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
