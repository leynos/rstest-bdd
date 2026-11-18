//! Regression coverage for the prepared CLI fixture.
//!
//! The family's input hash includes the workspace's Cargo configuration, so a
//! profile or linker change forces preparation instead of being absorbed by a
//! warm stamp. And the witness the preparation test asserts on — the fixture's
//! own test binaries in the profile — is shown to reject a tree that holds
//! unrelated artefacts instead, because a check that only ever runs against a
//! warm tree cannot distinguish a real preparation from a no-op.
//!
//! Declared beside `withdrawal` and `ownership` rather than inline in
//! `preparation.rs`, which keeps that file inside the repository's line budget.

use super::{
    FIXTURE_PACKAGE,
    cli_preparation,
    cli_preparation_description,
    closure::fixture_dependency_closure,
    fixture_dir,
    nested_cargo,
};
use crate::workspace_root;

/// The CLI family's inputs without the workspace Cargo configuration.
///
/// Kept beside the assertion so a future change to the family's roots makes
/// this list visibly stale rather than silently passing.
fn inputs_without_cargo_configuration() -> Vec<std::path::PathBuf> {
    let workspace = workspace_root();
    vec![
        fixture_dir(),
        workspace.join("Cargo.toml"),
        workspace.join("Cargo.lock"),
        workspace.join("crates"),
    ]
}

#[test]
fn the_workspace_cargo_configuration_is_an_input() {
    // The fixture build runs beneath the workspace, so Cargo applies the
    // committed `.cargo/config.toml`: its development profile options and
    // Linux linker flags rebuild dependency artefacts without changing a
    // fixture source. A stamp that ignored it would survive such an edit and
    // the timed CLI test would compile the graph inside its own 180 s limit.
    let (_environment, _target_root, inputs) =
        cli_preparation().expect("preparation description should resolve");
    let without = nested_cargo::input_hash(&inputs_without_cargo_configuration())
        .expect("hash without the workspace configuration");
    assert_ne!(
        inputs, without,
        "the workspace Cargo configuration must change the CLI input hash"
    );
}

#[test]
fn the_owned_packages_cover_the_whole_dependency_closure_and_reach_the_gate() {
    // Both halves of the fixture's build have to be declared, and the case
    // is written against the live graph rather than against a literal so it
    // cannot drift from the fixture. The workspace crates are the transitive
    // path dependencies the manifest never names; the registry crates are
    // `inventory`, `serial_test` and their trees, whose units are compiled
    // into the same profile and are evictable in exactly the same way even
    // though their sources cannot change.
    let closure = fixture_dependency_closure().expect("the fixture closure must resolve");
    let declared: std::collections::BTreeSet<&str> = std::iter::once(FIXTURE_PACKAGE)
        .chain(closure.iter().map(String::as_str))
        .collect();
    // The list the gate actually receives, not only a set rebuilt here from
    // the closure. A preparation that stopped wiring the closure would leave
    // every assertion below green — they test the local set — while the gate
    // filtered the dependency's records out of both sides and an eviction
    // passed. The fixture package is deliberately absent from
    // `owned_packages`: the experiment's own `cargo clean -p` removes it
    // before every baseline, so claiming it would defeat the preparation.
    let (preparation, _environment) =
        cli_preparation_description().expect("preparation description should resolve");
    let owned: std::collections::BTreeSet<&str> = preparation
        .owned_packages
        .iter()
        .map(String::as_str)
        .collect();
    let expected: std::collections::BTreeSet<&str> = closure.iter().map(String::as_str).collect();
    assert_eq!(
        owned, expected,
        "the preparation must hand the gate exactly the fixture's dependency closure, and the \
         fixture itself must not be among them"
    );

    // The registry half specifically: a list built from the manifests alone
    // holds the path crates and neither of these.
    for package in ["inventory", "serial_test"] {
        assert!(
            declared.contains(package),
            "the declaration must cover the fixture's registry dependencies; {package} is \
             compiled into the shared profile and its eviction would pass the gate"
        );
    }
    // And the transitive workspace half, which the manifests also omit.
    for package in [
        "rstest-bdd-harness",
        "rstest-bdd-patterns",
        "rstest-bdd-policy",
    ] {
        assert!(
            declared.contains(package),
            "the declaration must cover the fixture's transitive workspace dependencies; \
             {package} is built here and its eviction would pass the gate"
        );
    }
}
