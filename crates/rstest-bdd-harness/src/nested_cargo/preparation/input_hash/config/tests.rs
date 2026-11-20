//! Resolution of the walk's root, for roots that exist and roots that do
//! not.
//!
//! The production walk is a read of the tree, so its root is the one input
//! a caller decides rather than one the checkout fixes. These cases pin
//! that an absolute root is walked as given and that a relative one is
//! resolved to an absolute walk, which is what the fingerprint depends on.

use std::path::PathBuf;

use super::{cargo_config_inputs_with, search_root};

/// A checkout holding a `.cargo` directory, and a directory in it.
///
/// The paths are derived from the checkout's *resolved* form rather than
/// from the raw temporary directory. [`search_root`] canonicalizes, so the
/// probe reports every entry through that resolved form; on Windows that
/// carries the `\\?\` verbatim prefix, and no path built from the raw
/// spelling equals it. The cases below compare the probe's output against
/// these paths, so deriving them lexically made every Windows assertion
/// fail against a correct implementation.
///
/// The temporary directory is returned so it outlives the case; the paths
/// beside it are the resolved ones the assertions must use.
fn checkout_with_nested_directory() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let checkout = tempfile::tempdir().expect("checkout");
    let root = std::fs::canonicalize(checkout.path()).expect("resolved checkout");
    let nested = root.join("scratch/fixture");
    std::fs::create_dir_all(&nested).expect("nested directory");
    let cargo_dir = root.join(".cargo");
    std::fs::create_dir(&cargo_dir).expect("cargo dir");
    let config = cargo_dir.join("config.toml");
    std::fs::write(&config, "[build]\nrustflags = [\"-Copt-level=1\"]\n").expect("config");
    (checkout, nested, config, cargo_dir)
}

#[test]
fn an_absolute_root_keeps_its_own_ancestry() {
    // An absolute root already names one tree, so rooting it must not move
    // it. Without this, a fix that always joined the current directory
    // would still pass the case below by re-rooting everything.
    let (_checkout, nested, _config, _cargo_dir) = checkout_with_nested_directory();
    assert_eq!(
        search_root(&nested),
        nested,
        "an absolute root must be walked as given"
    );
}

#[test]
fn a_relative_root_resolves_to_an_absolute_walk() {
    // `Path::ancestors` is lexical. Walking `scratch/fixture` yields
    // `scratch/fixture`, `scratch` and the empty path, none of which is a
    // directory in the tree — so no configuration above the scratch root
    // could reach the fingerprint, and a stamp written from that result
    // survived an edit the nested build would notice.
    //
    // What this pins is the rule on its own: a relative root is joined to
    // the process directory so the walk has an absolute ancestry to climb.
    // The subtree here deliberately does *not* exist, which is both the
    // state the rule exists for — the preparation creates the tree it
    // stamps — and the state that keeps the *root* unresolvable.
    //
    // That is a statement about the root, not about its spelling. An absent
    // leaf does not stop `search_root` canonicalizing the longest *existing*
    // prefix above it, and on Windows `fs::canonicalize` returns the `\\?\`
    // verbatim form for every path it resolves, existing prefix or not. So
    // the walked root is verbatim-spelled on Windows while a lexical
    // `current_dir().join(..)` is not, and comparing the two directly made
    // this case fail against correct behaviour on both Windows lanes. The
    // resolved spelling is `checkout_with_nested_directory`'s subject, and
    // that case canonicalizes before deriving every expected path for the
    // same reason.
    //
    // The portable claim is that the walked root is the *resolved* process
    // directory joined to the same relative root, which is what
    // `split_at_existing_prefix` and `reappend` rebuild. Deriving the expected
    // value the same way compares the directory the root names, not the
    // spelling each side happens to use.
    let relative = PathBuf::from("a-directory-this-case-does-not-create");
    assert!(relative.is_relative(), "the case needs a relative root");
    assert!(
        !relative.exists(),
        "the case must exercise the fallback, not resolution"
    );

    let rooted = search_root(&relative);
    assert!(
        rooted.is_absolute(),
        "a walked root must be absolute, or its ancestry is not the tree's: {rooted:?}"
    );
    // The expected value is derived from the *resolved* process directory,
    // because that is the prefix `search_root` canonicalizes. Resolving the
    // walked root itself is not an option: the leaf is absent, and on Windows
    // `fs::canonicalize` opens the path, so it fails for anything that does
    // not exist. Resolving the process directory alone keeps the assertion
    // portable — it compares the directory the root names, not the spelling
    // each side happens to use.
    let resolved_process_directory =
        std::fs::canonicalize(std::env::current_dir().expect("a process directory"))
            .expect("the process directory resolves");
    assert_eq!(
        rooted,
        resolved_process_directory.join(&relative),
        "the fallback root is the process directory joined to the relative root, in the resolved \
         spelling the walk reports"
    );
}

#[test]
fn a_root_named_through_a_parent_contributes_the_same_walk_cold_and_warm() {
    // The hosted coverage-lane refusal, stated as an outcome. The family's
    // scratch root is spelled `<checkout>/crates/rstest-bdd/../..` so it is
    // stable across checkouts, and the preparation walks it *before* its
    // own callback creates the tree.
    //
    // While the walk resolved the root only when it existed, the same tree
    // produced two different input lists. Cold, the root kept its lexical
    // spelling and `Path::ancestors` contributed `crates`, `crates/..`,
    // `crates/rstest-bdd`, `crates/rstest-bdd/..` — four directories that
    // do not exist, one of which resolves to the checkout itself and so
    // reported the checkout's own `.cargo/config.toml` under a second
    // spelling. Warm, the root resolved and none of those were visited. The
    // preparation wrote a stamp for one list and the consumer demanded the
    // other, refusing a stamp whose artefacts were all present: the failure
    // reported an empty evicted list, which is the signature of a
    // fingerprint mismatch rather than missing artefacts.
    //
    // Resolving the longest existing prefix rather than the whole path
    // makes both runs agree. The assertion is on the *outcome* — the same
    // inputs, and so the same hash — rather than on how the root is
    // spelled, because the spelling is not what the stamp depends on.
    let checkout = tempfile::tempdir().expect("checkout");
    let root = std::fs::canonicalize(checkout.path()).expect("resolved checkout");
    let cargo_dir = root.join(".cargo");
    std::fs::create_dir(&cargo_dir).expect("cargo dir");
    let config = cargo_dir.join("config.toml");
    std::fs::write(&config, "[build]\nrustflags = [\"-Copt-level=1\"]\n").expect("config");

    // The defect's shape: an *existing* crate directory, then `..` back to
    // the checkout, then a tail that does not exist yet. The crate
    // directory has to exist for the lexical walk to keep climbing through
    // the `..` — and the `..` has to return to the checkout, so that the
    // lexical ancestry reaches the checkout's own `.cargo` and reports the
    // configuration a second time under the `other-crate/..` spelling.
    let crate_dir = root.join("other-crate");
    std::fs::create_dir(&crate_dir).expect("the crate directory");
    let through_parent = crate_dir.join("..").join("target/tests/family/fixture");
    assert!(
        !through_parent.exists(),
        "the cold half of the case needs an absent tail"
    );

    let cold = cargo_config_inputs_with(&through_parent, None).expect("cold inputs");

    std::fs::create_dir_all(&through_parent).expect("create the warm tree");
    let warm = cargo_config_inputs_with(&through_parent, None).expect("warm inputs");

    assert_eq!(
        cold, warm,
        "a root named through `..` must report the same configuration whether or not the tree \
         exists yet, or the preparation invalidates its own stamp"
    );
}

#[test]
fn configuration_above_the_root_reaches_the_inputs() {
    // The defect stated as an outcome rather than as a shape: the nested
    // build reads `<checkout>/.cargo/config.toml`, so a probe given the
    // nested directory must report that file. Before the walk was rooted it
    // reported nothing above the nested path.
    let (_checkout, nested, config, cargo_dir) = checkout_with_nested_directory();
    let roots = cargo_config_inputs_with(&nested, None).expect("config inputs");
    assert!(
        roots.contains(&config),
        "configuration above the root must reach the fingerprint: {roots:?}"
    );
    assert!(
        roots.iter().any(|path| path.starts_with(&cargo_dir)),
        "the checkout's own `.cargo` is visited on the way up: {roots:?}"
    );
}

#[test]
fn a_named_cargo_home_is_searched_last() {
    // Cargo reads `$CARGO_HOME/config.toml` in addition to the walk, and
    // the walk's own entries take precedence, so the home is consulted
    // after every ancestor rather than instead of them.
    let (_checkout, nested, config, _cargo_dir) = checkout_with_nested_directory();
    let home = tempfile::tempdir().expect("cargo home");
    let home_config = home.path().join("config.toml");
    std::fs::write(&home_config, "[build]\nrustflags = [\"-Copt-level=1\"]\n")
        .expect("home config");

    let roots = cargo_config_inputs_with(&nested, Some(home.path())).expect("config inputs");
    assert!(
        roots.contains(&config) && roots.contains(&home_config),
        "both the walk and the home contribute: {roots:?}"
    );
    assert_eq!(
        roots.last(),
        Some(&home_config),
        "the home is consulted after the walk, as Cargo does: {roots:?}"
    );
}
