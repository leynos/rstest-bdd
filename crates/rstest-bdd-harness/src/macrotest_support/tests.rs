//! Regression coverage for `TrybuildTree`: the inputs that invalidate a
//! prepared tree, and the `trybuild=` filter that must refuse preparation.

use std::fs;

use super::{TrybuildTree, trybuild_filter};

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
    let inputs = tree.inputs(&crate_root).expect("inputs");
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
    // The value is set for the duration of this case only; the helper below
    // scopes it, and `temp_env` serialises such cases within the binary.
    temp_env::with_var("CARGO_HOME", Some("cargo-home"), || {
        let inputs = tree.inputs(&generated).expect("inputs");
        assert!(
            inputs.contains(&generated_config),
            "a relative CARGO_HOME must be read from the generated crate the build runs in, not \
             from the workspace; the probe reported {inputs:?}"
        );
    });
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
