//! Regression coverage for `TrybuildTree`: the inputs that invalidate a
//! prepared tree, and the `trybuild=` filter that must refuse preparation.

use std::{ffi::OsStr, fs};

use super::{TrybuildTree, trybuild_filter};
use crate::nested_cargo;

#[test]
fn trybuild_inputs_include_the_workspace_cargo_configuration() {
    // `trybuild` builds the generated crate beneath the workspace, so Cargo
    // discovers the workspace `.cargo/config.toml` and applies its profile
    // and linker settings. A stamp that ignored it would survive a
    // configuration-only edit, and the timed case would compile the changed
    // graph inside its own deadline.
    let workspace = tempfile::tempdir().expect("workspace dir");
    // The input probe canonicalizes the directory it walks from, so every
    // configuration path it reports is spelled through the resolved form.
    // On Windows that carries a `\\?\` verbatim prefix and may shorten an
    // 8.3 temporary name, so a path built from the raw temporary directory
    // never equals the reported one. The fixture is therefore built from
    // the resolved root, which is the same tree.
    let root = fs::canonicalize(workspace.path()).expect("resolved workspace");
    let crate_root = root.join("crates/under-test");
    fs::create_dir_all(&crate_root).expect("crate dir");
    fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("manifest");
    fs::create_dir(root.join(".cargo")).expect("cargo dir");
    let config = root.join(".cargo/config.toml");
    fs::write(&config, "[profile.dev]\nincremental = false\n").expect("config");

    let tree = TrybuildTree::new(&crate_root, "under-test", "family", "variant");
    let inputs = tree.inputs_from(&crate_root, None).expect("inputs");
    assert!(
        inputs.contains(&config),
        "the workspace Cargo configuration must be a preparation input: {inputs:?}"
    );
}

#[test]
fn a_relative_cargo_home_is_read_from_the_generated_crate() {
    // The finding this covers: `trybuild` invokes Cargo inside the
    // generated crate, so a relative `CARGO_HOME` names a directory
    // *there*. Discovery rooted at the workspace would report the
    // workspace's copy instead — a different tree — and because the
    // generated crate lives under `target/`, which `input_hash` skips,
    // that mistake would leave the real Cargo home's configuration out of
    // the fingerprint entirely. An edit to it would then move dependency
    // compilation into a timed assertion.
    let workspace = tempfile::tempdir().expect("workspace dir");
    let root = fs::canonicalize(workspace.path()).expect("resolved workspace");
    let crate_root = root.join("crates/under-test");
    fs::create_dir_all(&crate_root).expect("crate dir");
    fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("manifest");

    // Two Cargo homes named by the same relative value: one beneath the
    // workspace, one beneath the generated crate the build runs in.
    let workspace_home = root.join("cargo-home");
    fs::create_dir_all(&workspace_home).expect("workspace cargo home");
    fs::write(
        workspace_home.join("config.toml"),
        "[build]\nrustflags = [\"-Copt-level=0\"]\n",
    )
    .expect("workspace home config");

    let generated = root.join("target/tests/trybuild/under-test");
    let generated_home = generated.join("cargo-home");
    fs::create_dir_all(&generated_home).expect("generated cargo home");
    let generated_config = generated_home.join("config.toml");
    fs::write(
        &generated_config,
        "[build]\nrustflags = [\"-Copt-level=1\"]\n",
    )
    .expect("generated home config");

    let tree = TrybuildTree::new(&crate_root, "under-test", "family", "variant");
    let inputs = tree
        .inputs_from(&generated, Some(OsStr::new("cargo-home")))
        .expect("inputs");
    assert!(
        inputs.contains(&generated_config),
        "a relative CARGO_HOME must be read from the generated crate the build runs in, not from \
         the workspace; the probe reported {inputs:?}"
    );
}

#[test]
fn a_rustflags_edit_in_the_generated_crate_refuses_the_stamp() {
    // The composition the path check above cannot make on its own: a stamp is
    // judged by the fingerprint, so a `CARGO_HOME` read from the right
    // directory still has to change the hash when a file in it changes.
    // Discovery rooted at the workspace would satisfy a path assertion about
    // the generated crate only if it reported it, and would leave the stamp
    // valid across an edit that rebuilds dependency artefacts.
    let workspace = tempfile::tempdir().expect("workspace dir");
    let root = fs::canonicalize(workspace.path()).expect("resolved workspace");
    let crate_root = root.join("crates/under-test");
    fs::create_dir_all(&crate_root).expect("crate dir");
    fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("manifest");

    let generated = root.join("target/tests/trybuild/under-test");
    let generated_home = generated.join("cargo-home");
    fs::create_dir_all(&generated_home).expect("generated cargo home");
    let generated_config = generated_home.join("config.toml");
    fs::write(
        &generated_config,
        "[build]\nrustflags = [\"-Copt-level=1\"]\n",
    )
    .expect("generated home config");

    let tree = TrybuildTree::new(&crate_root, "under-test", "family", "variant");
    let cargo_home = Some(OsStr::new("cargo-home"));
    let before = tree
        .fingerprint_from(&generated, cargo_home)
        .expect("fingerprint before");
    fs::write(
        &generated_config,
        "[build]\nrustflags = [\"-Copt-level=2\"]\n",
    )
    .expect("edited generated home config");
    let after = tree
        .fingerprint_from(&generated, cargo_home)
        .expect("fingerprint after");

    assert_ne!(
        before, after,
        "an edit to the Cargo home the nested build reads must invalidate the stamp"
    );
}

#[test]
fn configuration_between_the_generated_crate_and_the_workspace_is_an_input() {
    // Cargo climbs from the directory the command runs in, so a `.cargo`
    // directory anywhere between the generated crate and the workspace applies
    // to the nested build. None of them is reachable from a walk that starts at
    // the workspace, and `input_hash` skips a `target` directory during a walk,
    // so before the generated crate was walked too, an edit to one of these
    // files left the stamp matching while the build picked up new settings.
    let workspace = tempfile::tempdir().expect("workspace dir");
    let root = fs::canonicalize(workspace.path()).expect("resolved workspace");
    let crate_root = root.join("crates/under-test");
    fs::create_dir_all(&crate_root).expect("crate dir");
    fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("manifest");

    // The file the old walk could not see: beneath `target/`, above the
    // generated crate.
    let between = root.join("target/tests/trybuild");
    let between_cargo = between.join(".cargo");
    fs::create_dir_all(&between_cargo).expect("nested cargo dir");
    let between_config = between_cargo.join("config.toml");
    fs::write(
        &between_config,
        "[build]\nrustflags = [\"-Copt-level=1\"]\n",
    )
    .expect("nested config");

    let generated = between.join("under-test");
    fs::create_dir_all(&generated).expect("generated crate");

    let tree = TrybuildTree::new(&crate_root, "under-test", "family", "variant");
    let cargo_home = Some(OsStr::new("cargo-home"));
    let before = tree
        .fingerprint_from(&generated, cargo_home)
        .expect("fingerprint before");
    let inputs = tree.inputs_from(&generated, cargo_home).expect("inputs");
    assert!(
        inputs.contains(&between_config),
        "configuration between the generated crate and the workspace applies to the nested build \
         and must be a preparation input; the probe reported {inputs:?}"
    );

    fs::write(
        &between_config,
        "[build]\nrustflags = [\"-Copt-level=2\"]\n",
    )
    .expect("edited nested config");
    let after = tree
        .fingerprint_from(&generated, cargo_home)
        .expect("fingerprint after");

    assert_ne!(
        before, after,
        "an edit to configuration beneath the target directory must invalidate the stamp"
    );
}

#[test]
fn an_edit_beneath_the_target_directory_refuses_the_stamp_through_the_gate() {
    // The end-to-end half of the case above, and the one this finding asked
    // for: it drives a real stamp through the real gate rather than comparing
    // two hashes. A fingerprint that happened to change while the gate
    // compared something else would pass the hash assertion and still admit a
    // stale tree, so the refusal has to be shown at the boundary that decides.
    let workspace = tempfile::tempdir().expect("workspace dir");
    let root = fs::canonicalize(workspace.path()).expect("resolved workspace");
    let crate_root = root.join("crates/under-test");
    fs::create_dir_all(&crate_root).expect("crate dir");
    fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("manifest");

    let between = root.join("target/tests/trybuild");
    let between_cargo = between.join(".cargo");
    fs::create_dir_all(&between_cargo).expect("nested cargo dir");
    let between_config = between_cargo.join("config.toml");
    fs::write(
        &between_config,
        "[build]\nrustflags = [\"-Copt-level=1\"]\n",
    )
    .expect("nested config");

    let generated = between.join("under-test");
    fs::create_dir_all(&generated).expect("generated crate");
    // The gate judges a family by the units its profile holds, so the
    // generated crate needs the shape a real nested build leaves behind.
    write_pristine_profile(&generated);

    let tree = TrybuildTree::new(&crate_root, "under-test", "family", "variant");
    let cargo_home = Some(OsStr::new("cargo-home"));

    // Prepare a family whose inputs are the ones discovery reports, and stamp
    // it through the unconditional entry point the preparation test uses.
    let prepared = tree
        .scratch_preparation(&generated, cargo_home)
        .expect("the scratch family");
    nested_cargo::prepare_now(
        &prepared,
        || Ok(nested_cargo::PreparationSummary::default()),
    )
    .expect("the preparation should stamp the family");

    // A timed case against the same tree is admitted while nothing changed.
    admit(&prepared).expect("an unchanged tree must pass the gate");

    // Edit the configuration the old walk could not see, and the same timed
    // case must now refuse rather than build inside its own deadline.
    fs::write(
        &between_config,
        "[build]\nrustflags = [\"-Copt-level=2\"]\n",
    )
    .expect("edited nested config");
    let refreshed = tree
        .scratch_preparation(&generated, cargo_home)
        .expect("the refreshed family");
    let refused = admit(&refreshed).expect_err("an edited configuration must refuse the stamp");
    assert_eq!(refused.kind(), std::io::ErrorKind::NotFound);
}

/// Judge `preparation` the way a timed `nextest` case does.
///
/// A timed case may not build: if the stamp is refused it has already lost the
/// deadline it exists to protect. The panic makes that a test failure rather
/// than a silent cold build inside the assertion.
fn admit(preparation: &nested_cargo::Preparation) -> std::io::Result<()> {
    nested_cargo::ensure_prepared_building(preparation, true, || {
        panic!("a timed case must not prepare its own family")
    })
    .map(|_outcome| ())
}

/// Write the compiled units the gate requires into `crate_root`'s profile.
///
/// A family is admissible only when the profile beneath it holds the units the
/// stamp records, so a fixture that wrote none would be refused for missing
/// artefacts rather than for the fingerprint this case is about. The shape is
/// the one rustc leaves: a `.d` record paired with the unit it describes.
fn write_pristine_profile(crate_root: &std::path::Path) {
    let profile = crate_root.join("target/debug");
    let deps = profile.join("deps");
    fs::create_dir_all(&deps).expect("deps dir");
    fs::create_dir_all(profile.join(".fingerprint/unit-hash")).expect("fingerprint dir");
    fs::write(
        profile.join(".fingerprint/unit-hash/unit"),
        b"fingerprint record",
    )
    .expect("fingerprint record");
    fs::write(deps.join("unit.d"), b"dep-info").expect("dep info");
    fs::write(deps.join("libunit.rlib"), b"unit").expect("compiled unit");
}

#[test]
fn preparation_refuses_a_trybuild_filter() {
    // `trybuild` retains only the queued fixtures whose path contains the
    // filter substring, so a filtered preparation would queue both probes
    // and run a subset of them — then write the complete stamp anyway. That
    // stamp would admit the timed case into a tree missing the mode it did
    // not warm, which is the cold-build failure issue 910 records.
    let error = trybuild_filter([
        "nested_fixture_preparation".to_owned(),
        "trybuild=scenario_attributes".to_owned(),
    ])
    .expect_err("a filtered preparation must be refused");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    let message = error.to_string();
    assert!(
        message.contains("trybuild=scenario_attributes"),
        "the refusal must name the offending filter: {message}"
    );
}

#[test]
fn preparation_accepts_an_unfiltered_run() {
    // The common case: the runner's own arguments, and possibly a test-name
    // filter that `trybuild` ignores. Neither may be read as a fixture
    // filter, and a bare `trybuild=` prefix with no suffix is not one.
    for arguments in [
        vec!["nested_fixture_preparation".to_owned()],
        vec![
            "nested_fixture_preparation".to_owned(),
            "trybuild=".to_owned(),
        ],
        vec![
            "nested_fixture_preparation".to_owned(),
            "--exact".to_owned(),
        ],
    ] {
        assert!(
            trybuild_filter(arguments.clone()).is_ok(),
            "an unfiltered run must prepare: {arguments:?}"
        );
    }
}

#[test]
fn the_target_root_query_is_offline_and_skips_dependencies() {
    // The query reads only `target_directory`. Resolving the dependency graph
    // would be pure cost, and an online resolution would make a preparation
    // stamp depend on the network — the state this suite is meant to avoid.
    let command = super::metadata_command(std::path::Path::new("Cargo.toml"));
    let cargo = command.cargo_command();
    let args = cargo
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let offline = cargo
        .get_envs()
        .find(|(key, _)| *key == OsStr::new("CARGO_NET_OFFLINE"))
        .and_then(|(_, value)| value);

    assert!(
        args.iter().any(|arg| arg == "--no-deps"),
        "the target-root query must not resolve dependencies; got {args:?}"
    );
    assert_eq!(
        offline,
        Some(OsStr::new("true")),
        "the target-root query must run offline"
    );
}
