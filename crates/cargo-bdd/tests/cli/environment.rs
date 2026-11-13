//! Environment policy for the CLI fixture's child process.
//!
//! Every smoke test spawns `cargo bdd` through the map these tests build, so
//! the map decides which inherited variables can reach the binary. Two rules
//! matter and neither is visible from a passing smoke test: a caller-provided
//! `CARGO_TARGET_DIR` must survive so nested builds reuse the workspace's
//! compiled units, and any inherited `CARGO_BDD_*` variable must be dropped
//! because the binary reads that prefix as a configuration tier.

use std::ffi::OsString;

use super::{cli_child_environment, workspace_root};

/// The child keeps a caller target directory and otherwise uses the workspace.
#[rstest::rstest]
#[case::workspace_cache(None)]
#[case::caller_cache(Some("custom-coverage-target"))]
fn cli_child_environment_reuses_target(#[case] target: Option<&str>) -> std::io::Result<()> {
    let vars = target
        .map(|value| vec![(OsString::from("CARGO_TARGET_DIR"), OsString::from(value))])
        .unwrap_or_default();
    let child = cli_child_environment(vars)?;
    // A caller's value is preserved as a *directory*, not necessarily as the
    // exact string: a relative value names a different place to a child that
    // runs in the fixture directory, so the helper resolves it against this
    // process's working directory before handing it over. The workspace
    // fallback is already absolute.
    let expected = target.map_or_else(
        || workspace_root().join("target"),
        |value| std::env::current_dir().expect("current dir").join(value),
    );
    assert_eq!(child.get("CARGO_TARGET_DIR"), Some(expected.as_os_str()));
    assert_eq!(
        child
            .target_directory()
            .expect("a resolved environment should yield its target directory")
            .as_deref(),
        Some(expected.as_path()),
        "the resolved target directory must be the absolute one the child receives"
    );
    Ok(())
}

/// The child cannot see a host configuration file through a discovery root.
///
/// Discovery merges `$HOME/.cargo_bdd.toml`, the XDG bases and, on Windows,
/// the home dotfile. A root left inherited is enough for a developer's own
/// file to change a smoke test's output; measured with a `HOME` dotfile
/// setting `cmds.skipped.json = true`, a listing test fails. Each root must
/// therefore name a directory that exists and holds nothing discovery looks
/// for, and `XDG_CONFIG_DIRS` must be set rather than absent because absent
/// means `/etc/xdg`.
#[cfg(unix)]
#[test]
fn cli_child_environment_redirects_every_discovery_root() {
    let child = cli_child_environment(Vec::new());

    let mut roots = Vec::new();
    for variable in ["HOME", "XDG_CONFIG_HOME", "XDG_CONFIG_DIRS"] {
        let Some(value) = child.get(variable) else {
            panic!("{variable} must be redirected, not inherited");
        };
        let path = std::path::Path::new(value);
        assert!(path.is_dir(), "{variable} must name an existing directory");
        roots.push(path.to_path_buf());
    }

    for path in &roots {
        let entries = std::fs::read_dir(path).expect("the redirected root should be readable");
        assert_eq!(
            entries.count(),
            0,
            "{} must stay empty so no configuration can be discovered",
            path.display()
        );
    }
    let distinct: std::collections::HashSet<_> = roots.iter().collect();
    assert_eq!(
        distinct.len(),
        roots.len(),
        "each root must name its own directory, or one redirect would shadow another"
    );
}

/// Redirection keeps the nested build's registry and toolchain resolvable.
///
/// Cargo derives `CARGO_HOME` and `RUSTUP_HOME` from the home directory this
/// policy overrides, so both must still be present and must still name the
/// ambient locations rather than the empty directory.
#[test]
fn cli_child_environment_keeps_cargo_homes_out_of_the_redirect() {
    let child = cli_child_environment(Vec::new());
    let home = child.get("HOME").expect("HOME is redirected");

    for (variable, directory) in [("CARGO_HOME", ".cargo"), ("RUSTUP_HOME", ".rustup")] {
        let value = child
            .get(variable)
            .unwrap_or_else(|| panic!("{variable} must be carried into the child"));
        let path = std::path::Path::new(value);
        assert!(
            !path.starts_with(std::path::Path::new(home)),
            "{variable} must not point inside the empty redirect directory"
        );
        assert_eq!(path.file_name(), Some(OsStr::new(directory)));
    }
}

/// A developer's own `CARGO_BDD_*` export never reaches the child.
///
/// Without the filter the child would merge that value over the file a case is
/// testing, so the file-tier assertions could pass or fail for the wrong
/// reason. Both a real setting name and an unrelated name in the same prefix
/// are checked, because the binary reads the whole prefix as configuration.
#[test]
fn cli_child_environment_drops_inherited_configuration_variables() -> std::io::Result<()> {
    let vars = vec![
        (
            OsString::from("CARGO_BDD_CMDS_STEPS_SKIPPED"),
            OsString::from("true"),
        ),
        (OsString::from("CARGO_BDD_LOG"), OsString::from("info")),
        (
            OsString::from("CARGO_TARGET_DIR"),
            OsString::from("kept-target"),
        ),
    ];

    let child = cli_child_environment(vars)?;

    assert_eq!(child.get("CARGO_BDD_CMDS_STEPS_SKIPPED"), None);
    assert_eq!(child.get("CARGO_BDD_LOG"), None);
    // The surviving value is resolved against this process's directory, which
    // is why the assertion compares a path rather than the literal string.
    let expected_target = std::env::current_dir()
        .expect("current dir")
        .join("kept-target");
    assert_eq!(
        child.get("CARGO_TARGET_DIR"),
        Some(expected_target.as_os_str())
    );
    assert_eq!(
        child
            .target_directory()
            .expect("a resolved environment should yield its target directory")
            .as_deref(),
        Some(expected_target.as_path())
    );
    Ok(())
}

/// `OsStr` is referenced through the imported alias above.
#[allow(dead_code)]
fn _assert_os_str_alias(_: &OsStr) {}
