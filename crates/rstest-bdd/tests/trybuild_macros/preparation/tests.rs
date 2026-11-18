//! Regression coverage for the inputs that invalidate a prepared tree.
//!
//! Exercises [`input_roots`] from the parent [`preparation`](super) module
//! and the hash the gate derives from it. The parent declares the two
//! families this binary prepares — the `trybuild` tree and the Clippy UI
//! target — and both stamps are only as good as the roots they hashed, so
//! what is checked here is that a root which changes a nested build is
//! actually in the list. The cases below take one such root at a time and
//! show it reaching `nested_cargo::input_hash`, because a root that never
//! reaches the hash is the same as one omitted from the list.

use rstest_bdd_harness::nested_cargo;

use super::input_roots;

#[test]
fn the_workspace_cargo_configuration_is_an_input() {
    // Both nested builds run beneath the workspace, so Cargo applies the
    // committed `.cargo/config.toml`: its profile options and Linux linker
    // flags change dependency artefacts without changing a fixture source.
    // A stamp that ignored it would survive a configuration-only edit and
    // the timed assertion would compile the graph instead.
    //
    // Removing the root from the same list, rather than hashing a different
    // tree, isolates the configuration's contribution: every other input is
    // identical between the two hashes.
    let roots = input_roots().expect("input roots");
    let workspace_config = super::workspace_root()
        .join(".cargo/config.toml")
        .into_std_path_buf();
    assert!(
        roots.contains(&workspace_config),
        "expected {} among the preparation inputs: {roots:?}",
        workspace_config.display()
    );

    let with = nested_cargo::input_hash(&roots).expect("hash with configuration");
    let without: Vec<std::path::PathBuf> = roots
        .into_iter()
        .filter(|root| root != &workspace_config)
        .collect();
    let without = nested_cargo::input_hash(&without).expect("hash without configuration");
    assert_ne!(
        with, without,
        "the workspace Cargo configuration must change the input hash"
    );
}
