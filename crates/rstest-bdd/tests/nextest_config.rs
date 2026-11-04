//! Integration contract for the workspace's nextest override and timeout budget
//! policy.
//!
//! The suite parses `.config/nextest.toml` with `toml` and asserts its timeout
//! policy, including `global-timeout` and the `cargo-spawning` thread limit.
//!
//! Four contracts are pinned:
//!
//! * `nextest_override_preserves_timeout_contract` covers the `cargo-bdd` test binaries at 180 s
//!   and the feature-rebuild binary at 600 s. Both overrides retain their termination settings and
//!   `cargo-spawning` membership.
//! * `trybuild_nextest_override_preserves_timeout_contract` requires exactly one override to name
//!   any of the four compile-test binaries, and that override's filter to name all four and no
//!   others. It then requires the override to keep a 20 m `slow-timeout` period, `terminate-after =
//!   1`, a 5 s grace period and membership of the `cargo-spawning` group, and the default profile's
//!   `global-timeout` to exceed the period in seconds.
//! * `nested_cargo_run_retains_cold_build_reserve` recomputes the R-6 sizing scenario (a reference
//!   lane of `180 + 600 * 4 + 600 * 3` seconds, ten cold nested invocations at 19 s each and a 600
//!   s reserve) and requires the default profile's `global-timeout` to cover it, and the
//!   `cargo-spawning` group's `max-threads` to stay at 1 so nested builds keep serializing.
//!
//! The overrides, the group and the run-level ceiling are policy that a future
//! edit could break silently, so these assertions fail loudly instead. They
//! cover the two nextest tiers of the four-tier timeout budget: a per-test
//! `slow-timeout` compared against the whole-run `global-timeout`. The
//! coverage watchdog and the job `timeout-minutes` are the outer two tiers,
//! covered by the workflow contracts under `tests/workflow_contracts`;
//! `.config/nextest.toml` asks that suite and this file to be kept in step.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    io,
    path::{Path, PathBuf},
};

use cap_std::{ambient_authority, fs::Dir};
use rstest::rstest;
use toml::Value;

const EXPECTED_TRYBUILD_BINARIES: [&str; 4] = [
    "rstest-bdd-harness-tokio::macro_compile",
    "rstest-bdd-harness-gpui::macro_compile",
    "rstest-bdd::trybuild_macros",
    "rstest-bdd-server::workspace_discovery_compile",
];
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
        "the default profile's global timeout ({global_timeout}) must exceed the trybuild slow \
         timeout ({period}); otherwise nextest kills the run before the test the budget exists \
         for can finish, and raising the budget alone rescues nothing",
    );
    Ok(())
}

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

/// Keep cold-build drift from consuming the run-level reserve again.
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

fn nextest_configuration() -> Result<Value, Box<dyn Error>> {
    let configuration_directory = workspace_root()?.join(".config");
    let configuration_directory =
        Dir::open_ambient_dir(configuration_directory, ambient_authority())?;
    let source = configuration_directory.read_to_string("nextest.toml")?;

    Ok(toml::from_str(&source)?)
}

fn workspace_root() -> Result<PathBuf, io::Error> {
    let manifest_directory = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_directory
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| {
            io::Error::other(
                "rstest-bdd manifest directory must be nested under the workspace root",
            )
        })?;

    Ok(workspace_root.to_path_buf())
}

fn default_profile_key<'a>(configuration: &'a Value, key: &str) -> Option<&'a Value> {
    configuration
        .get("profile")
        .and_then(Value::as_table)
        .and_then(|profiles| profiles.get("default"))
        .and_then(Value::as_table)
        .and_then(|default_profile| default_profile.get(key))
}

fn default_global_timeout(configuration: &Value) -> Result<&str, io::Error> {
    default_profile_key(configuration, "global-timeout")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            io::Error::other("nextest configuration must set profile.default.global-timeout")
        })
}

/// `test-groups` is a top-level nextest table, not a profile key, so it needs
/// its own lookup rather than `default_profile_key`.
fn test_group_max_threads(configuration: &Value, group_name: &str) -> Result<i64, io::Error> {
    configuration
        .get("test-groups")
        .and_then(Value::as_table)
        .and_then(|groups| groups.get(group_name))
        .and_then(Value::as_table)
        .and_then(|group| group.get("max-threads"))
        .and_then(Value::as_integer)
        .ok_or_else(|| {
            io::Error::other(format!(
                "nextest configuration must set test-groups.{group_name}.max-threads"
            ))
        })
}

fn duration_seconds(duration: &str) -> Result<u64, io::Error> {
    let unit_start = duration
        .find(|character: char| !character.is_ascii_digit())
        .ok_or_else(|| io::Error::other(format!("duration {duration} names no unit")))?;
    let (value, unit) = duration.split_at(unit_start);
    let value: u64 = value
        .parse()
        .map_err(|_| io::Error::other(format!("duration {duration} names no count")))?;
    let multiplier = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3_600,
        _ => {
            return Err(io::Error::other(format!(
                "duration {duration} uses unit {unit}"
            )));
        }
    };

    value
        .checked_mul(multiplier)
        .ok_or_else(|| io::Error::other(format!("duration {duration} overflows")))
}

fn default_overrides(configuration: &Value) -> Result<&[Value], io::Error> {
    let overrides = default_profile(configuration)?
        .get("overrides")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            io::Error::other("nextest configuration must contain profile.default.overrides")
        })?;

    Ok(overrides)
}

fn default_profile(configuration: &Value) -> Result<&toml::value::Table, io::Error> {
    configuration
        .get("profile")
        .and_then(Value::as_table)
        .and_then(|profiles| profiles.get("default"))
        .and_then(Value::as_table)
        .ok_or_else(|| io::Error::other("nextest configuration must contain profile.default"))
}
fn trybuild_override_candidates(overrides: &[Value]) -> Vec<usize> {
    overrides
        .iter()
        .enumerate()
        .filter_map(|(index, override_value)| {
            let filter = override_value.get("filter")?.as_str()?;
            binary_ids(filter)
                .iter()
                .any(|binary_id| EXPECTED_TRYBUILD_BINARIES.contains(binary_id))
                .then_some(index)
        })
        .collect()
}

fn binary_id_report(overrides: &[Value], candidates: &[usize]) -> String {
    let binary_ids = candidates.iter().flat_map(|index| {
        overrides
            .get(*index)
            .and_then(|override_value| override_value.get("filter"))
            .and_then(Value::as_str)
            .map(binary_ids)
            .unwrap_or_default()
    });
    let mut occurrences = BTreeMap::new();
    for binary_id in binary_ids {
        *occurrences.entry(binary_id).or_insert(0_usize) += 1;
    }

    let expected = BTreeSet::from(EXPECTED_TRYBUILD_BINARIES);
    let actual = occurrences.keys().copied().collect::<BTreeSet<_>>();
    let missing = expected
        .difference(&actual)
        .copied()
        .collect::<BTreeSet<_>>();
    let duplicate = occurrences
        .iter()
        .filter_map(|(binary_id, count)| (*count > 1).then_some(*binary_id))
        .collect::<BTreeSet<_>>();
    let unexpected = actual
        .difference(&expected)
        .copied()
        .collect::<BTreeSet<_>>();

    format!(
        "missing binary IDs: {}; duplicate binary IDs: {}; unexpected binary IDs: {}",
        format_binary_ids(&missing),
        format_binary_ids(&duplicate),
        format_binary_ids(&unexpected),
    )
}

fn binary_ids(filter: &str) -> Vec<&str> {
    filter
        .split("binary_id(")
        .skip(1)
        .filter_map(|suffix| suffix.split_once(')').map(|(binary_id, _)| binary_id))
        .collect()
}

fn format_binary_ids(binary_ids: &BTreeSet<&str>) -> String {
    if binary_ids.is_empty() {
        "<none>".to_owned()
    } else {
        binary_ids.iter().copied().collect::<Vec<_>>().join(", ")
    }
}
