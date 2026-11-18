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
    cli_preparation_description,
    closure::fixture_dependency_closure,
    family_roots,
    nested_cargo,
};
use crate::workspace_root;

#[test]
fn the_workspace_cargo_configuration_is_an_input() {
    // The fixture build runs beneath the workspace, so Cargo applies the
    // committed `.cargo/config.toml`: its development profile options and
    // Linux linker flags rebuild dependency artefacts without changing a
    // fixture source. A stamp that ignored it would survive such an edit and
    // the timed CLI test would compile the graph inside its own 180 s limit.
    //
    // Both sides are the family's *own* root list, so a root the family
    // stopped naming disappears from both and the case fails rather than
    // passing on a restated copy. A restated control would keep its own copy
    // of every root, and the hashes would then differ for a reason unrelated
    // to the configuration.
    let workspace = workspace_root();
    let roots = family_roots().expect("the family's roots should resolve");
    // Both sides are cut down by the same predicate, and the only root they
    // differ in is the committed `.cargo` one. Dropping every non-workspace
    // root instead — the obvious control — also removes `$CARGO_HOME/config.toml`
    // on a host that has one, and a family that stopped hashing the workspace
    // file would still report two different hashes: the case would pass over
    // the regression it exists to catch. Removing the Cargo home entry from
    // both sides keeps that one root as the single difference, without copying
    // a file outside the checkout.
    let committed = workspace.join(".cargo");
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".cargo"))
        });
    let shared = |root: &std::path::PathBuf| match &cargo_home {
        Some(home) => !root.starts_with(home),
        None => true,
    };
    let workspace_configuration: Vec<std::path::PathBuf> = roots
        .iter()
        .filter(|root| root.starts_with(&committed))
        .cloned()
        .collect();
    assert!(
        workspace_configuration
            .iter()
            .any(|path| path.ends_with("config.toml")),
        "this checkout commits `.cargo/config.toml`, so the family must name it; the roots were \
         {roots:?}"
    );
    let control: Vec<std::path::PathBuf> =
        roots.iter().filter(|root| shared(root)).cloned().collect();
    let without: Vec<std::path::PathBuf> = control
        .iter()
        .filter(|root| !root.starts_with(&committed))
        .cloned()
        .collect();
    let with_hash =
        nested_cargo::input_hash(&control).expect("hash with the workspace configuration");
    let without_hash =
        nested_cargo::input_hash(&without).expect("hash without the workspace configuration");
    assert_ne!(
        with_hash, without_hash,
        "the workspace Cargo configuration must change the CLI input hash; removing \
         {workspace_configuration:?} left it unchanged"
    );
}

#[test]
fn the_timed_graph_query_keeps_its_own_smaller_budget() {
    // The graph query runs before the gate has read the stamp, so a timed
    // assertion pays for it even when the fixture is prepared. The preparation
    // budget is nine minutes, and this binary's period is 180 s: under the
    // preparation bound a contended query could not return before nextest
    // killed the test, discarding the captured child output that names the
    // wait. The two bounds must therefore stay distinct, and this is the
    // comparison that keeps a later edit from re-merging them.
    //
    // The `.config/nextest.toml` period is not parsed here — this crate has no
    // TOML reader among its dev-dependencies — so the 180 s figure below is
    // the CLI override's period, restated. Keeping the two bounds apart is
    // what the assertion pins; a change to either figure belongs beside the
    // configuration entry that sets it, and must be mirrored here.
    //
    // Only the query's own bound is checked against the period, not the
    // query's bound plus the gate's. The two are not additive: a query that
    // spends its whole budget returns an error and the gate returns before
    // the identity probe runs, so the worst case a test can reach is one of
    // them, not both.
    let query = super::closure::TIMED_GRAPH_QUERY_TIMEOUT.as_secs();
    let preparation = nested_cargo::PREPARATION_COMMAND_TIMEOUT.as_secs();
    let cli_period = 180;
    assert!(
        query < preparation,
        "the timed graph query must keep a budget below the preparation test's {preparation}s, or \
         nextest kills the test before the query can report its own timeout"
    );
    assert!(
        query < cli_period,
        "a {query}s query under the CLI override's {cli_period}s period would be killed by \
         nextest before it could report its own timeout"
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
    let closure = fixture_dependency_closure(super::closure::TIMED_GRAPH_QUERY_TIMEOUT)
        .expect("the fixture closure must resolve");
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
        cli_preparation_description(super::closure::TIMED_GRAPH_QUERY_TIMEOUT)
            .expect("preparation description should resolve");
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
