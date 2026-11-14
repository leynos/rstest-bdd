//! Cargo's own configuration as a preparation input.
//!
//! Cargo discovers configuration by walking up from a nested build's working
//! directory, so the workspace `.cargo/config.toml` applies to every fixture
//! build here. It sets development-profile options and the Linux linker flags,
//! and changing either changes dependency artefacts without changing a single
//! fixture source. A stamp that ignored it would survive a configuration-only
//! edit, and the timed assertion would then compile the graph the preparation
//! was meant to have built.
//!
//! The cases split by the shape the configuration takes on disk: an ordinary
//! file, a symbolic link to one, a dangling link, and a path that exists but
//! cannot be inspected. Each of them has to reach the hash, and the failure
//! modes have to be reported rather than read as an absence.

use std::path::PathBuf;

use crate::nested_cargo::{cargo_config_inputs, input_hash};

/// A workspace root with a `.cargo` directory, and the configuration link in
/// it.
fn workspace_with_cargo_dir() -> (tempfile::TempDir, PathBuf) {
    let workspace = tempfile::tempdir().expect("workspace dir");
    std::fs::create_dir(workspace.path().join(".cargo")).expect("cargo dir");
    let config = workspace.path().join(".cargo/config.toml");
    (workspace, config)
}

#[test]
fn cargo_configuration_is_an_input() {
    let (workspace, config) = workspace_with_cargo_dir();
    std::fs::write(&config, "[profile.dev]\nincremental = false\n").expect("config");

    assert_eq!(
        cargo_config_inputs(workspace.path()).expect("config inputs"),
        vec![config.clone()]
    );

    let before = input_hash(&cargo_config_inputs(workspace.path()).expect("config inputs"))
        .expect("hash before");
    std::fs::write(&config, "[profile.dev]\nincremental = true\n").expect("config edit");
    let after = input_hash(&cargo_config_inputs(workspace.path()).expect("config inputs"))
        .expect("hash after");
    assert_ne!(
        before, after,
        "a configuration-only change must invalidate the prepared artefacts"
    );
}

#[test]
fn missing_cargo_configuration_is_not_an_input() {
    // A checkout may commit neither accepted file name; naming a missing root
    // would make every preparation fail on a read error instead of treating the
    // configuration as absent.
    let workspace = tempfile::tempdir().expect("workspace dir");
    assert_eq!(
        cargo_config_inputs(workspace.path()).expect("config inputs"),
        [] as [PathBuf; 0]
    );
}

#[test]
#[cfg(unix)]
fn a_symlinked_cargo_configuration_contributes_its_target() {
    // `symlink_metadata` classifies a link as neither file nor directory, so
    // hashing the entry alone contributed nothing and a `.cargo/config.toml`
    // committed as a symlink was absent from the fingerprint. Editing the file
    // it points at — a profile option, a linker flag — would then leave the
    // stamp matching, and the timed assertion would compile the graph the
    // preparation was meant to have built. The link is Unix-only because
    // creating one on Windows needs a privilege the test runner lacks.
    let (workspace, link) = workspace_with_cargo_dir();
    let target: PathBuf = workspace.path().join("shared-config.toml");
    std::fs::write(&target, "[profile.dev]\nincremental = false\n").expect("config target");
    std::os::unix::fs::symlink(&target, &link).expect("config symlink");

    let roots = cargo_config_inputs(workspace.path()).expect("config inputs");
    assert_eq!(roots, vec![link], "the link must still be reported");

    let before = input_hash(&roots).expect("hash through the link");
    std::fs::write(&target, "[profile.dev]\nincremental = true\n").expect("config edit");
    let after = input_hash(&roots).expect("hash through the link");
    assert_ne!(
        before, after,
        "editing the file a configuration link points at must invalidate the prepared artefacts"
    );
}

#[test]
#[cfg(unix)]
fn a_dangling_cargo_configuration_link_reads_as_absent() {
    // `cargo_config_inputs` follows the link, so a target that is gone is a
    // `NotFound` and the configuration is treated as absent. That is safe in
    // both directions because the transition changes the hash either way: a
    // stamp written while the link dangled covers no configuration, and it
    // stops matching as soon as the target appears; one written with the
    // target present stops matching when the target goes away. Reporting it
    // as a read failure would instead make every preparation on such a tree
    // fail, which is why the absent answer is the one pinned here.
    let (workspace, link) = workspace_with_cargo_dir();
    std::os::unix::fs::symlink(workspace.path().join("gone.toml"), &link).expect("symlink");

    assert_eq!(
        cargo_config_inputs(workspace.path()).expect("a dangling link is not an error"),
        [] as [PathBuf; 0],
        "a link whose target is gone contributes no configuration"
    );
}

#[test]
#[cfg(unix)]
fn an_unreadable_cargo_configuration_is_an_error() {
    // A configuration path that exists but cannot be inspected must not be
    // reported as absent. Reporting it absent would drop the configuration
    // from the fingerprint, so a stamp would survive a change the build
    // notices — exactly the invalidation this input exists to force.
    //
    // A symlink loop reaches that state without any permission setup, which
    // would not hold when the suite runs as root. It is a Unix-only fixture
    // because creating a symlink on Windows needs a privilege the test runner
    // does not have.
    let (workspace, looped) = workspace_with_cargo_dir();
    std::os::unix::fs::symlink(&looped, &looped).expect("symlink loop");

    let error = cargo_config_inputs(workspace.path())
        .expect_err("an uninspectable configuration must not pass as absent");
    assert_ne!(
        error.kind(),
        std::io::ErrorKind::NotFound,
        "a loop is not an absence: {error}"
    );
}
