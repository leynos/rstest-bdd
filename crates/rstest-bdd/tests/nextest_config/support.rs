//! Readers over the parsed `.config/nextest.toml` value.
//!
//! The contracts live in the parent module; these helpers locate the keys they
//! read and report the binary IDs an override's filter names. Keeping the
//! lookups here keeps the module that states the policy readable.

use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::{Path, PathBuf},
};

use cap_std::{ambient_authority, fs::Dir};
use toml::Value;

/// The test binaries that compile a trybuild fixture tree.
pub(super) const EXPECTED_TRYBUILD_BINARIES: [&str; 4] = [
    "rstest-bdd-harness-tokio::macro_compile",
    "rstest-bdd-harness-gpui::macro_compile",
    "rstest-bdd::trybuild_macros",
    "rstest-bdd-server::workspace_discovery_compile",
];

/// Parse `.config/nextest.toml` from the workspace root.
pub(super) fn nextest_configuration() -> Result<Value, Box<dyn std::error::Error>> {
    let configuration_directory = workspace_root()?.join(".config");
    let configuration_directory =
        Dir::open_ambient_dir(configuration_directory, ambient_authority())?;
    let source = configuration_directory.read_to_string("nextest.toml")?;

    Ok(toml::from_str(&source)?)
}

/// The workspace root, two levels above this crate's manifest directory.
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

/// The value of `key` in the named profile, if that profile sets it.
fn profile_key<'a>(configuration: &'a Value, profile_name: &str, key: &str) -> Option<&'a Value> {
    configuration
        .get("profile")
        .and_then(Value::as_table)
        .and_then(|profiles| profiles.get(profile_name))
        .and_then(Value::as_table)
        .and_then(|profile| profile.get(key))
}

/// The named profile's run-level `global-timeout`.
///
/// Generalizing the default-profile reader is what lets the long profile be
/// checked against the same ordering, rather than duplicating the lookup and
/// its dotted-path error for a second profile name.
pub(super) fn profile_global_timeout<'a>(
    configuration: &'a Value,
    profile_name: &str,
) -> Result<&'a str, io::Error> {
    profile_key(configuration, profile_name, "global-timeout")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            io::Error::other(format!(
                "nextest configuration must set profile.{profile_name}.global-timeout"
            ))
        })
}

/// The default profile's run-level `global-timeout`.
pub(super) fn default_global_timeout(configuration: &Value) -> Result<&str, io::Error> {
    profile_global_timeout(configuration, "default")
}

/// `test-groups` is a top-level nextest table, not a profile key, so it needs
/// its own lookup rather than `default_profile_key`.
pub(super) fn test_group_max_threads(
    configuration: &Value,
    group_name: &str,
) -> Result<i64, io::Error> {
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

/// The top-level `nextest-version` the configuration requires.
///
/// nextest ignores a `priority` setting in a file that does not require a
/// version new enough to read it, which would silently make the preparation
/// override ordinary. Reading the version here is what catches that.
pub(super) fn nextest_version(configuration: &Value) -> Result<(u64, u64, u64), io::Error> {
    let version = configuration
        .get("nextest-version")
        .and_then(Value::as_str)
        .ok_or_else(|| io::Error::other("nextest configuration must set nextest-version"))?;
    let mut parts = version.split('.');
    let mut component = || -> Result<u64, io::Error> {
        parts
            .next()
            .and_then(|part| part.parse().ok())
            .ok_or_else(|| io::Error::other(format!("nextest-version {version} is not x.y.z")))
    };
    Ok((component()?, component()?, component()?))
}

/// Convert a nextest duration string (`30m`, `600s`, `1h`) to seconds.
pub(super) fn duration_seconds(duration: &str) -> Result<u64, io::Error> {
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

/// The named profile's ordered overrides.
///
/// A profile other than `default` lists only its own overrides here; nextest
/// checks them before `profile.default.overrides`, so a caller asking what
/// governs a test under that profile reads this list first.
pub(super) fn profile_overrides<'a>(
    configuration: &'a Value,
    profile_name: &str,
) -> Result<&'a [Value], io::Error> {
    profile_key(configuration, profile_name, "overrides")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| {
            io::Error::other(format!(
                "nextest configuration must contain profile.{profile_name}.overrides"
            ))
        })
}

/// The default profile's ordered overrides.
pub(super) fn default_overrides(configuration: &Value) -> Result<&[Value], io::Error> {
    profile_overrides(configuration, "default")
}

/// Indices of the overrides whose filter names a trybuild test binary.
pub(super) fn trybuild_override_candidates(overrides: &[Value]) -> Vec<usize> {
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

/// Report the binary IDs the candidate overrides name, split by how they
/// diverge from [`EXPECTED_TRYBUILD_BINARIES`].
pub(super) fn binary_id_report(overrides: &[Value], candidates: &[usize]) -> String {
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

/// The binary IDs a `binary_id(...)` nextest filter names.
fn binary_ids(filter: &str) -> Vec<&str> {
    filter
        .split("binary_id(")
        .skip(1)
        .filter_map(|suffix| suffix.split_once(')').map(|(binary_id, _)| binary_id))
        .collect()
}

/// Render a binary-ID set for a failure message.
fn format_binary_ids(binary_ids: &BTreeSet<&str>) -> String {
    if binary_ids.is_empty() {
        "<none>".to_owned()
    } else {
        binary_ids.iter().copied().collect::<Vec<_>>().join(", ")
    }
}

#[cfg(test)]
mod tests {
    //! Coverage for the profile reader's selection and error paths.

    use toml::Value;

    use super::profile_global_timeout;

    /// The reader returns the profile it is asked for, not the default one.
    ///
    /// The two profiles carry different values so a reader that always selects
    /// `default` fails here rather than agreeing with a shared expectation.
    #[test]
    fn profile_global_timeout_reads_the_requested_profile() {
        let configuration: Value = toml::from_str(concat!(
            "[profile.default]\n",
            "global-timeout = \"90m\"\n",
            "[profile.long]\n",
            "global-timeout = \"30m\"\n",
        ))
        .expect("valid TOML");

        assert_eq!(
            profile_global_timeout(&configuration, "default").expect("default profile"),
            "90m",
            "the default profile's own global timeout must be returned"
        );
        assert_eq!(
            profile_global_timeout(&configuration, "long").expect("long profile"),
            "30m",
            "the long profile's own global timeout must be returned, not the default's"
        );
    }

    /// A missing leaf, a wrongly typed one and an absent profile all name the
    /// dotted path, including for a profile other than `default`.
    ///
    /// The Windows strict coverage ratchet reads this path, so each rejection
    /// is exercised rather than left to a real misconfiguration.
    #[test]
    fn profile_global_timeout_reports_unreadable_leaves_with_their_path() {
        let cases = [
            ("[profile.default]\n", "default", "a missing leaf"),
            (
                "[profile.default]\nglobal-timeout = 10\n",
                "default",
                "a wrongly typed leaf",
            ),
            (
                "[profile.default]\nglobal-timeout = \"90m\"\n",
                "long",
                "an absent profile",
            ),
        ];

        for (document, profile_name, label) in cases {
            let configuration: Value = toml::from_str(document).expect("valid TOML");
            let error = profile_global_timeout(&configuration, profile_name)
                .expect_err("an unreadable global-timeout must be reported");
            assert_eq!(
                error.to_string(),
                format!("nextest configuration must set profile.{profile_name}.global-timeout"),
                "{label} must name the dotted path that could not be read"
            );
        }
    }
}
