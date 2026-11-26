//! The `local` profile's contracts.
//!
//! Every nextest invocation in the Makefile selects `profile.local`: both
//! passes of `make test`, `make prepare-nested-fixtures` and `make
//! nextest-run`. It exists because a development host whose compile-admission
//! queue is hundreds of requests deep spends longer waiting for compile slots
//! than the default profile allows, and the default profile cannot grow: its
//! `global-timeout` is the innermost of the CI timeout tiers.
//!
//! The profile only helps if nextest reaches its overrides and if every limit
//! it sets is longer than the default one it replaces. An override shadowed by
//! an earlier match, or a value copied from the default, would leave the local
//! gate failing exactly where it failed before, so both are asserted here.
//!
//! # Why this is a separate module
//!
//! The parent module states the default profile's policy, and `preparation`
//! states the default preparation override's share of it. This profile is a
//! third subject: it overrides both, and its limits are compared against both.

use std::{error::Error, io};

use toml::Value;

use super::{
    preparation::{
        DEFAULT_PREPARATION_PERIOD,
        assert_preparation_budget,
        single_preparation_override,
    },
    support::{
        binary_id_report,
        default_global_timeout,
        default_overrides,
        duration_seconds,
        nextest_configuration,
        profile_global_timeout,
        profile_overrides,
        trybuild_override_candidates,
    },
};

/// The profile the Makefile's nextest recipes select.
const LOCAL_PROFILE: &str = "local";

/// The preparation period under [`LOCAL_PROFILE`].
const LOCAL_PREPARATION_PERIOD: &str = "240m";

/// The compile-test period under [`LOCAL_PROFILE`].
const LOCAL_COMPILE_TEST_PERIOD: &str = "120m";

/// The run-level budget under [`LOCAL_PROFILE`].
const LOCAL_GLOBAL_TIMEOUT: &str = "360m";

/// The local preparation override is reached first and outlasts the default.
#[test]
fn local_profile_outlasts_the_default_preparation_budget() -> Result<(), Box<dyn Error>> {
    let configuration = nextest_configuration()?;
    let overrides = profile_overrides(&configuration, LOCAL_PROFILE)?;
    let override_value = single_preparation_override(overrides)?;
    assert_preparation_budget(override_value, LOCAL_PREPARATION_PERIOD)?;
    assert_eq!(
        override_value.get("priority").and_then(Value::as_integer),
        Some(100),
        "the local profile's preparation override must keep the default override's priority",
    );

    let local_period = duration_seconds(LOCAL_PREPARATION_PERIOD)?;
    let default_period = duration_seconds(DEFAULT_PREPARATION_PERIOD)?;
    assert!(
        local_period > default_period,
        "the local preparation period ({local_period}s) must exceed the default one \
         ({default_period}s), or the local profile rescues nothing",
    );
    assert_run_outlasts(&configuration, local_period, "preparation")
}

/// The local compile-test override names the same four binaries as the
/// default one, and outlasts it.
#[test]
fn local_profile_outlasts_the_default_compile_test_budget() -> Result<(), Box<dyn Error>> {
    let configuration = nextest_configuration()?;
    let overrides = profile_overrides(&configuration, LOCAL_PROFILE)?;
    let local_override = single_trybuild_override(overrides)?;
    let default_override = single_trybuild_override(default_overrides(&configuration)?)?;

    let slow_timeout = local_override
        .get("slow-timeout")
        .and_then(Value::as_table)
        .ok_or_else(|| io::Error::other("the local compile-test override must set slow-timeout"))?;
    assert_eq!(
        slow_timeout.get("period").and_then(Value::as_str),
        Some(LOCAL_COMPILE_TEST_PERIOD),
        "the local compile-test override must allow {LOCAL_COMPILE_TEST_PERIOD}",
    );
    assert_eq!(
        slow_timeout
            .get("terminate-after")
            .and_then(Value::as_integer),
        Some(1),
        "the local compile-test override must stay a hang detector (terminate-after = 1)",
    );
    assert_eq!(
        local_override.get("test-group").and_then(Value::as_str),
        Some("cargo-spawning"),
        "the local compile-test override must stay in the cargo-spawning group",
    );

    let local_period = duration_seconds(LOCAL_COMPILE_TEST_PERIOD)?;
    let default_period = default_override
        .get("slow-timeout")
        .and_then(|slow_timeout| slow_timeout.get("period"))
        .and_then(Value::as_str)
        .ok_or_else(|| io::Error::other("the default compile-test override must set a period"))
        .and_then(duration_seconds)?;
    assert!(
        local_period > default_period,
        "the local compile-test period ({local_period}s) must exceed the default one \
         ({default_period}s), or the local profile rescues nothing",
    );
    assert_run_outlasts(&configuration, local_period, "compile-test")
}

/// The one override in `overrides` naming the trybuild binaries, naming all of
/// them and nothing else.
fn single_trybuild_override(overrides: &[Value]) -> Result<&Value, Box<dyn Error>> {
    let candidates = trybuild_override_candidates(overrides);
    let report = binary_id_report(overrides, &candidates);
    assert_eq!(
        candidates.len(),
        1,
        "expected exactly one compile-test override in the profile. {report}",
    );
    assert_eq!(
        report,
        "missing binary IDs: <none>; duplicate binary IDs: <none>; unexpected binary IDs: <none>",
        "the compile-test override must name exactly the four compile-test binaries. {report}",
    );
    candidates
        .first()
        .and_then(|index| overrides.get(*index))
        .ok_or_else(|| io::Error::other("expected one compile-test override").into())
}

/// The local run budget is the pinned value, exceeds the default one, and
/// exceeds `period_seconds`, so nextest never ends the run before a test
/// spends its own budget.
fn assert_run_outlasts(
    configuration: &Value,
    period_seconds: u64,
    budget: &str,
) -> Result<(), Box<dyn Error>> {
    let global_timeout = profile_global_timeout(configuration, LOCAL_PROFILE)?;
    assert_eq!(
        global_timeout, LOCAL_GLOBAL_TIMEOUT,
        "the local profile must allow a {LOCAL_GLOBAL_TIMEOUT} run, found {global_timeout}",
    );
    let local_global = duration_seconds(global_timeout)?;
    let default_global = duration_seconds(default_global_timeout(configuration)?)?;
    assert!(
        local_global > default_global && local_global > period_seconds,
        "the local run budget ({local_global}s) must exceed the default one ({default_global}s) \
         and the local {budget} period ({period_seconds}s)",
    );
    Ok(())
}
