//! Integration contract for the workspace's nextest override and timeout budget
//! policy.
//!
//! The suite parses `.config/nextest.toml` with `toml` and asserts its timeout
//! policy, including `global-timeout` and the `cargo-spawning` thread limit.
//!
//! Five contracts are pinned:
//!
//! * `preparation_nextest_override_preserves_timeout_contract` requires exactly one override to
//!   match `test(/nested_fixture_preparation$/)`, at a 30 m period with the same termination
//!   settings and group, and to be scheduled ahead of every other override (`priority` 100 against
//!   the default 0). It also requires the top-level `nextest-version` that `priority` needs, and
//!   that the preparation override is the first one in the file, so the first-match precedence rule
//!   keeps it authoritative if a future override ever matched the same tests.
//! * `nextest_override_preserves_timeout_contract` covers the `cargo-bdd` test binaries at 180 s
//!   and the feature-rebuild binary at 600 s. Both overrides retain their termination settings and
//!   `cargo-spawning` membership.
//! * `trybuild_nextest_override_preserves_timeout_contract` requires exactly one override to name
//!   any of the four compile-test binaries, and that override's filter to name all four and no
//!   others. It then requires the override to keep a 20 m `slow-timeout` period, `terminate-after =
//!   1`, a 5 s grace period and membership of the `cargo-spawning` group, and the default profile's
//!   `global-timeout` to exceed the period in seconds.
//! * `preparation_command_bound_fires_before_the_runner_kills_the_test` requires the per-command
//!   preparation timeout multiplied by the number of sequential commands a preparation test may run
//!   to stay below the preparation override's period, so the named timeout error that carries the
//!   child's captured output fires first.
//! * `nested_cargo_run_retains_cold_build_reserve` recomputes the R-6 sizing scenario (a reference
//!   lane of `180 + 600 * 4 + 600 * 3` seconds, ten cold nested invocations at 19 s each and a 600
//!   s reserve) and requires the default profile's `global-timeout` to cover it, and the
//!   `cargo-spawning` group's `max-threads` to stay at 1 so nested builds keep serializing.
//!
//! The timed overrides are asserted at their exact periods rather than only
//! against the global timeout, because widening one is how a cold build gets
//! hidden instead of moved: the preparation override is the only place in the
//! file where a cold dependency build may spend a test's budget.
//!
//! Four binaries now carry a longer period: the preparation, the `cargo-bdd`
//! CLI smoke tests, `rstest-bdd-harness::binary_test_support_cargo`, and the
//! compile-test and rebuild-invalidation families. Each was added because a
//! lane observation showed it shelling out to `cargo` under the default 60 s
//! hang detector; `binary_test_support_cargo` was the last one found, on run
//! 43607129223. A fifth nested-Cargo test must join this set rather than
//! inherit the default.
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
//!
//! The preparation override's own contracts — that it is the only match, that
//! it comes first, how it is scheduled, and its per-command bound — live in
//! `#[path = "nextest_config/preparation.rs"]`, for the same reason and to keep
//! this module inside its file-length budget. The five contracts listed above
//! are still pinned by this test binary; two of them are asserted there.

use std::{error::Error, io};

use rstest::rstest;
use toml::Value;

#[path = "nextest_config/support.rs"]
mod support;
use support::{
    binary_id_report,
    default_global_timeout,
    default_overrides,
    duration_seconds,
    nextest_configuration,
    test_group_max_threads,
    trybuild_override_candidates,
};

#[path = "nextest_config/preparation.rs"]
mod preparation;

const FEATURE_REBUILD_FILTER: &str = "binary_id(rstest-bdd::feature_rebuild_invalidation)";
const CARGO_BDD_FILTER: &str = "binary_id(cargo-bdd::cli) | binary_id(cargo-bdd::invocation)";

/// Expected timeout settings owned by this integration test.
///
/// Keep these expectations local to this module; they describe the workspace's
/// nextest policy rather than application configuration.
#[derive(Clone, Copy)]
struct TimeoutExpectation {
    test_name: &'static str,
    period: &'static str,
}

/// An exact nextest filter paired with the timeout it must preserve.
#[derive(Clone, Copy)]
struct FilteredTimeoutExpectation {
    filter: &'static str,
    timeout: TimeoutExpectation,
}

const CARGO_BDD_TIMEOUT_EXPECTATION: FilteredTimeoutExpectation = FilteredTimeoutExpectation {
    filter: CARGO_BDD_FILTER,
    timeout: TimeoutExpectation {
        test_name: "cargo-bdd",
        period: "180s",
    },
};
const FEATURE_REBUILD_TIMEOUT_EXPECTATION: FilteredTimeoutExpectation =
    FilteredTimeoutExpectation {
        filter: FEATURE_REBUILD_FILTER,
        timeout: TimeoutExpectation {
            test_name: "feature-rebuild",
            period: "600s",
        },
    };
const TRYBUILD_TIMEOUT_EXPECTATION: TimeoutExpectation = TimeoutExpectation {
    test_name: "trybuild",
    period: "20m",
};

/// Each filtered override keeps its termination policy and group.
#[rstest]
#[case::cargo_bdd(CARGO_BDD_TIMEOUT_EXPECTATION)]
#[case::feature_rebuild(FEATURE_REBUILD_TIMEOUT_EXPECTATION)]
fn nextest_override_preserves_timeout_contract(
    #[case] expectation: FilteredTimeoutExpectation,
) -> Result<(), Box<dyn Error>> {
    let configuration = nextest_configuration()?;
    let overrides = default_overrides(&configuration)?;
    let override_value = override_for_exact_filter(overrides, expectation)?;
    assert_slow_timeout_contract(override_value, expectation.timeout)?;
    Ok(())
}

/// The trybuild override keeps its 20 m period, termination and group.
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

    let period = assert_slow_timeout_contract(trybuild_override, TRYBUILD_TIMEOUT_EXPECTATION)?;

    let global_timeout = default_global_timeout(&configuration)?;
    assert!(
        duration_seconds(global_timeout)? > duration_seconds(period)?,
        "the default profile's global timeout ({global_timeout}) must exceed the trybuild slow timeout ({period}); otherwise nextest kills the run before the test the budget exists for can finish, and raising the budget alone rescues nothing",
    );
    Ok(())
}

/// Find the single override whose filter matches exactly.
fn override_for_exact_filter(
    overrides: &[Value],
    expectation: FilteredTimeoutExpectation,
) -> Result<&Value, io::Error> {
    let matching = overrides
        .iter()
        .filter(|override_value| {
            override_value.get("filter").and_then(Value::as_str) == Some(expectation.filter)
        })
        .collect::<Vec<_>>();
    let [override_value] = matching.as_slice() else {
        return Err(io::Error::other(format!(
            "{} nextest override for {:?} must match exactly once; found {}",
            expectation.timeout.test_name,
            expectation.filter,
            matching.len()
        )));
    };
    Ok(*override_value)
}

/// Assert the slow-timeout period and termination settings of an override.
fn assert_slow_timeout_contract(
    override_value: &Value,
    expectation: TimeoutExpectation,
) -> Result<&str, io::Error> {
    let slow_timeout = override_value
        .get("slow-timeout")
        .and_then(Value::as_table)
        .ok_or_else(|| {
            io::Error::other(format!(
                "{} override must contain a slow-timeout table",
                expectation.test_name
            ))
        })?;
    let period = slow_timeout
        .get("period")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            io::Error::other(format!(
                "{} slow-timeout table must contain a period",
                expectation.test_name
            ))
        })?;
    assert_eq!(
        period, expectation.period,
        "{} override must use the expected slow-timeout period",
        expectation.test_name
    );
    assert_eq!(
        slow_timeout
            .get("terminate-after")
            .and_then(Value::as_integer),
        Some(1),
        "{} override must terminate after one slow-timeout period",
        expectation.test_name
    );
    assert_eq!(
        slow_timeout.get("grace-period").and_then(Value::as_str),
        Some("5s"),
        "{} override must retain its five-second grace period",
        expectation.test_name
    );
    assert_eq!(
        override_value.get("test-group").and_then(Value::as_str),
        Some("cargo-spawning"),
        "{} override must remain in the cargo-spawning group",
        expectation.test_name
    );
    Ok(period)
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
