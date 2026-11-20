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

use crate::nested_cargo::{
    cargo_config_inputs,
    input_hash,
    preparation::{cargo_config_inputs_with, resolve_cargo_home},
};

/// A workspace root with a `.cargo` directory, and the configuration link in
/// it.
fn workspace_with_cargo_dir() -> std::io::Result<(tempfile::TempDir, PathBuf, PathBuf)> {
    let workspace = tempfile::tempdir()?;
    // The probe canonicalizes the root before walking it, so every path it
    // reports is spelled through the resolved form. On Windows that carries a
    // `\\?\` verbatim prefix and may shorten `RUNNER~1` to `runneradmin`, so a
    // path built from the raw temporary directory never equals the reported
    // one. Deriving the expected path from the resolved root is what makes the
    // assertion portable; the check is that the file is *found*, not that it
    // is spelled a particular way.
    let root = std::fs::canonicalize(workspace.path())?;
    std::fs::create_dir(root.join(".cargo"))?;
    let config = root.join(".cargo/config.toml");
    Ok((workspace, root, config))
}

#[test]
fn cargo_configuration_is_an_input() {
    let (_workspace, root, config) = workspace_with_cargo_dir().expect("the workspace fixture");
    std::fs::write(&config, "[profile.dev]\nincremental = false\n").expect("config");

    // The workspace file is asserted by presence rather than by whole-vector
    // equality: the probe also reports configuration above the workspace and in
    // `CARGO_HOME`, which is host state a portable test cannot enumerate.
    let roots = cargo_config_inputs(&root).expect("config inputs");
    assert!(
        roots.contains(&config),
        "the workspace Cargo configuration must be an input: {roots:?}"
    );

    let before =
        input_hash(&cargo_config_inputs(&root).expect("config inputs")).expect("hash before");
    std::fs::write(&config, "[profile.dev]\nincremental = true\n").expect("config edit");
    let after =
        input_hash(&cargo_config_inputs(&root).expect("config inputs")).expect("hash after");
    assert_ne!(
        before, after,
        "a configuration-only change must invalidate the prepared artefacts"
    );
}

#[test]
fn a_cargo_home_setting_change_invalidates_the_prepared_tree() {
    // Cargo reads `$CARGO_HOME/config.toml` in addition to its upward walk, so
    // a setting there — a rustflags override, a target-directory override —
    // changes what the nested build produces. While that file was outside the
    // input list, editing it left the stamp matching and the timed assertion
    // compiled the graph the preparation was meant to have built.
    //
    // Both environment variables are process-wide and this suite runs in
    // parallel, so the Cargo home is supplied to the probe rather than written
    // into the process. A case that assigned `CARGO_HOME` changed what every
    // other thread in this binary read: `cargo_configuration_is_an_input`
    // hashed a directory another case had already deleted, and failed with
    // `NotFound` on a path it had just created.
    let (_workspace, root, _workspace_config) =
        workspace_with_cargo_dir().expect("the workspace fixture");
    // An *absolute* Cargo home is used unchanged, and the probe reports it
    // through the spelling the caller supplied, so the lexical temporary path
    // is the right one to expect here.
    let cargo_home = tempfile::tempdir().expect("cargo home");
    let home_config = cargo_home.path().join("config.toml");
    std::fs::write(&home_config, "[build]\nrustflags = [\"-Copt-level=1\"]\n")
        .expect("home config");

    let roots = cargo_config_inputs_with(&root, Some(cargo_home.path())).expect("config inputs");
    assert!(
        roots.contains(&home_config),
        "the CARGO_HOME configuration must be a preparation input: {roots:?}"
    );
    let before = input_hash(&roots).expect("hash before");
    std::fs::write(&home_config, "[build]\nrustflags = [\"-Copt-level=3\"]\n")
        .expect("home config edit");
    let after = input_hash(&roots).expect("hash after");
    assert_ne!(
        before, after,
        "a CARGO_HOME-only change must invalidate the prepared artefacts"
    );
}

#[test]
fn a_relative_cargo_home_is_resolved_against_the_nested_directory() {
    // Cargo interprets a relative `CARGO_HOME` against the working directory
    // of the command that reads it. The nested build runs beneath the
    // workspace, so `CARGO_HOME=.cargo-cache` names `<workspace>/.cargo-cache`,
    // while this process would read its own directory. The two agree only if
    // the probe resolves the value the same way, and until it did, editing the
    // fixture's own Cargo home left a warm stamp matching and moved the
    // compilation back into the timed test.
    //
    // The value is resolved by `cargo_home` against the workspace, and the
    // resolution is reached here by passing it through the injectable form, so
    // the case cannot disturb a sibling that reads the environment.
    let workspace = tempfile::tempdir().expect("workspace dir");
    // The relative home is resolved against the *caller's* spelling of the
    // workspace and then used unchanged, so both the fixture and the
    // expectation are built from that same lexical root.
    let workspace_root = std::fs::canonicalize(workspace.path()).expect("resolved workspace");
    let nested_home = workspace_root.join(".cargo-cache");
    let nested_config = nested_home.join("config.toml");
    std::fs::create_dir_all(&nested_home).expect("nested cargo home");
    std::fs::write(&nested_config, "[build]\nrustflags = [\"-Copt-level=1\"]\n")
        .expect("nested config");

    let relative = resolve_cargo_home(
        &workspace_root,
        Some(std::ffi::OsStr::new(".cargo-cache")),
        || Some(std::path::PathBuf::from("/home/unused")),
    )
    .expect("a relative CARGO_HOME names a directory");
    let roots = cargo_config_inputs_with(&workspace_root, Some(&relative)).expect("config inputs");
    assert!(
        roots.contains(&nested_config),
        "a relative CARGO_HOME must be read from the nested build's directory; the probe reported \
         {roots:?} instead of {}",
        nested_config.display()
    );
}

#[test]
fn an_absolute_cargo_home_is_used_unchanged() {
    // The complement of the relative case: an absolute `CARGO_HOME` already
    // names one directory for both the probe and the build, so resolving it
    // must not re-root it beneath the workspace.
    let workspace = tempfile::tempdir().expect("workspace dir");
    let cargo_home = tempfile::tempdir().expect("cargo home");
    let config = cargo_home.path().join("config.toml");
    std::fs::write(&config, "[build]\nrustflags = [\"-Copt-level=1\"]\n").expect("config");

    let roots =
        cargo_config_inputs_with(workspace.path(), Some(cargo_home.path())).expect("config inputs");
    assert!(
        roots.contains(&config),
        "an absolute CARGO_HOME must be read as given: {roots:?}"
    );
}

#[test]
fn missing_cargo_configuration_is_not_an_input() {
    // A checkout may commit neither accepted file name; naming a missing root
    // would make every preparation fail on a read error instead of treating the
    // configuration as absent.
    //
    // The probe now also reports any configuration above the workspace and in
    // `CARGO_HOME`, so the assertion is about the workspace's own entries
    // rather than about an empty vector: on a host with a `~/.cargo/config.toml`
    // there is legitimately something to report, and a whole-vector equality
    // would fail for that reason rather than for the behaviour under test.
    let workspace = tempfile::tempdir().expect("workspace dir");
    // Both sides are resolved: the probe reports through the canonical root,
    // and a lexical `starts_with` against it would be false for every entry.
    let root = std::fs::canonicalize(workspace.path()).expect("resolved workspace");
    let roots = cargo_config_inputs(&root).expect("config inputs");
    assert!(
        !roots.iter().any(|path| path.starts_with(&root)),
        "a workspace with no committed configuration contributes nothing: {roots:?}"
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
    let (_workspace, root, link) = workspace_with_cargo_dir().expect("the workspace fixture");
    let target: PathBuf = root.join("shared-config.toml");
    std::fs::write(&target, "[profile.dev]\nincremental = false\n").expect("config target");
    std::os::unix::fs::symlink(&target, &link).expect("config symlink");

    let roots = cargo_config_inputs(&root).expect("config inputs");
    assert!(
        roots.contains(&link),
        "the link must still be reported: {roots:?}"
    );

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
    let (_workspace, root, link) = workspace_with_cargo_dir().expect("the workspace fixture");
    std::os::unix::fs::symlink(root.join("gone.toml"), &link).expect("symlink");

    let roots = cargo_config_inputs(&root).expect("a dangling link is not an error");
    assert!(
        !roots.contains(&link),
        "a link whose target is gone contributes no configuration: {roots:?}"
    );
}

#[test]
fn relocating_identical_configuration_invalidates_the_prepared_tree() {
    // The name and the bytes of a configuration file are not the whole of what
    // it means. `build.rustc = "tools/rustc"`, `build.target-dir` and the
    // relative `CARGO_HOME` form all resolve against the configuration's own
    // directory, so the same file moved one level up can select a different
    // compiler or a different build directory while leaving name and content
    // byte-identical. A hash built from those two alone would survive the move
    // and admit a timed build that recompiles the graph.
    //
    // Both sides are hashed through `input_hash` as *named roots*, which is how
    // `cargo_config_inputs` supplies them, so the case exercises the rule the
    // production path uses rather than a hand-built equivalent.
    // A closure rather than a `Vec` of digests: the two are compared pairwise,
    // and an index would be the only reason to collect them.
    let named_config_hash = |directory: &std::path::Path| {
        std::fs::create_dir(directory.join(".cargo")).expect("cargo dir");
        let config = directory.join(".cargo/config.toml");
        std::fs::write(&config, "[build]\nrustc = \"tools/rustc\"\n").expect("identical config");
        input_hash(&[config]).expect("hash of the named configuration")
    };
    let first = tempfile::tempdir().expect("first directory");
    let second = tempfile::tempdir().expect("second directory");
    assert_ne!(
        named_config_hash(first.path()),
        named_config_hash(second.path()),
        "an identical configuration named from a different directory must invalidate the prepared \
         artefacts, because its path-valued settings resolve against that directory"
    );
}

#[test]
fn fixture_tree_identities_stay_checkout_relative() {
    // The complement of the case above, and the reason the provenance is
    // recorded only for a *named root*. A fixture tree is hashed by the names
    // the tree gives its entries, so the same sources in a different checkout
    // must produce the same digest. Recording absolute paths for every entry
    // would make each stamp depend on where the repository was cloned: a
    // second worktree would prepare again with nothing changed, and a stamp
    // copied between checkouts would be refused for a difference in a path
    // component the build never reads.
    let checkout_hash = |checkout: &std::path::Path| {
        let source = checkout.join("src");
        std::fs::create_dir(&source).expect("source dir");
        std::fs::write(source.join("lib.rs"), "pub fn f() {}\n").expect("source");
        input_hash(&[source]).expect("hash of the fixture tree")
    };
    let first = tempfile::tempdir().expect("first checkout");
    let second = tempfile::tempdir().expect("second checkout");
    assert_eq!(
        checkout_hash(first.path()),
        checkout_hash(second.path()),
        "a fixture tree must hash by its own names, not by where the checkout lives"
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
    let (_workspace, root, looped) = workspace_with_cargo_dir().expect("the workspace fixture");
    std::os::unix::fs::symlink(&looped, &looped).expect("symlink loop");

    let error = cargo_config_inputs(&root)
        .expect_err("an uninspectable configuration must not pass as absent");
    assert_ne!(
        error.kind(),
        std::io::ErrorKind::NotFound,
        "a loop is not an absence: {error}"
    );
}

#[test]
fn a_scratch_directories_configuration_is_an_input() {
    // The nested builds run in a scratch copy beneath the target directory,
    // and Cargo's configuration walk starts there rather than at the
    // workspace. A configuration the walk reaches from the scratch directory —
    // between it and the checkout — sets profile options or a linker the build
    // reads and the workspace's own inputs never name.
    //
    // The families that prepare against these builds therefore hash the scratch
    // directory's configuration in addition to the workspace's. This case pins
    // the composition: a change reachable only from the scratch root must move
    // the fingerprint, or a stamp written before the edit survives it and the
    // dependency rebuild lands inside the timed test.
    let scratch = tempfile::tempdir().expect("scratch root");
    // Resolved for the same reason as `workspace_with_cargo_dir`: the probe
    // canonicalizes the root, so the path it reports is the resolved spelling
    // and not the raw temporary one.
    let root = std::fs::canonicalize(scratch.path()).expect("resolved scratch root");
    let cargo_dir = root.join(".cargo");
    std::fs::create_dir(&cargo_dir).expect("cargo dir");
    let config = cargo_dir.join("config.toml");
    std::fs::write(&config, "[profile.dev]\nincremental = false\n").expect("config");

    let roots = cargo_config_inputs(&root).expect("config inputs");
    assert!(
        roots.contains(&config),
        "the scratch directory's own configuration must be an input: {roots:?}"
    );

    let before = input_hash(&roots).expect("hash before");
    std::fs::write(&config, "[profile.dev]\nincremental = true\n").expect("config edit");
    let after =
        input_hash(&cargo_config_inputs(&root).expect("config inputs")).expect("hash after");
    assert_ne!(
        before, after,
        "a scratch-only configuration change must invalidate the prepared artefacts"
    );
}
