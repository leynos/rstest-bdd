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

use std::path::Path;

use rstest_bdd_harness::nested_cargo;

use super::{input_roots, ui_clippy::ui_clippy_preparation};

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
    // `workspace_root` resolves and `cargo_config_inputs` reports through that
    // resolved form, so the configuration arrives as `.cargo/config.toml`
    // beneath the same resolved root rather than as this lexical join. Asking
    // by containment states the question the case means — is this file among
    // the inputs — and holds whichever form the probe happens to use, which is
    // what the plain equality here missed on the Windows lane: the probe
    // reported a `\\?\` verbatim path that no plainly spelled entry equals.
    let cargo_dir = super::workspace_root().join(".cargo").into_std_path_buf();
    let is_configuration = |root: &std::path::PathBuf| root.starts_with(&cargo_dir);
    assert!(
        roots.iter().any(is_configuration),
        "expected the workspace Cargo configuration beneath {} among the preparation inputs: \
         {roots:?}",
        cargo_dir.display()
    );

    let with = nested_cargo::input_hash(&roots).expect("hash with configuration");
    // Cut down by the same predicate the assertion above reads with, so the two
    // sides differ in exactly the configuration and in nothing else.
    let without: Vec<std::path::PathBuf> = roots
        .into_iter()
        .filter(|root| !is_configuration(root))
        .collect();
    let without = nested_cargo::input_hash(&without).expect("hash without configuration");
    assert_ne!(
        with, without,
        "the workspace Cargo configuration must change the input hash"
    );
}

#[test]
fn the_ui_lint_family_probes_from_its_manifest_directory() {
    // The UI-lint family is the one whose stamped root and working directory
    // are deliberately different: `ui_clippy_command` runs `cargo clippy` from
    // `CARGO_MANIFEST_DIR` while the tree it builds lives under
    // `target/tests/ui_lints_clippy`. The toolchain probe has to run from the
    // same directory, because Cargo resolves a relative `RUSTC` and a relative
    // `CARGO_HOME` against the command's directory — a probe left in the
    // stamped root would identify a different compiler than the lint cases
    // use, and the stamp would vouch for that wrong identity.
    let preparation = ui_clippy_preparation().expect("the UI-lint family");
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        preparation.working_directory.as_path(),
        manifest_dir,
        "the probe must run where the lint command runs, not where it stamps"
    );
    assert_eq!(
        preparation.artefacts,
        vec![manifest_dir.join("target/tests/ui_lints_clippy")],
        "the stamped root keeps its own path, which is not the working directory"
    );
}
