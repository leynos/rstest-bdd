//! Regression coverage for where the preparation stamp lives.
//!
//! Exercises this family's `fixture_preparation` and `workspace_inputs`
//! helpers together with the harness's `prepare_now`, `ensure_prepared_with`
//! and `input_hash`, over the child environment the parent module resolves. A
//! stamp is only meaningful if it is evicted together with the artefacts it
//! vouches for. An inherited `CARGO_TARGET_DIR` moves those artefacts out of
//! the workspace `target/`, so a stamp stored in the scratch root would survive
//! an eviction of the external directory, keep matching, and let the timed
//! assertion pay for a cold dependency compile — the exact failure issue 910
//! removes. A sibling `ownership` module covers the family's declaration of
//! which packages its build produced.

use std::ffi::OsString;

use cap_std::ambient_authority;
use rstest::fixture;
use rstest_bdd_harness::nested_cargo;

use super::{REBUILD, fixture_preparation, workspace_inputs};
use crate::harness::{fixtures, process};

// The families' ownership declaration, exercised against what their fixtures
// actually resolve to. Its own module because it is a separate claim from the
// stamp-location regressions above.
#[path = "tests/ownership.rs"]
mod ownership;

/// A child environment whose artefacts go to `target_directory`.
fn child_env(target_directory: &std::path::Path) -> std::io::Result<process::ChildEnv> {
    process::child_env_from_vars(vec![(
        OsString::from("CARGO_TARGET_DIR"),
        target_directory.as_os_str().to_os_string(),
    )])
}

/// Leave `env`'s target directory as a real build would: a profile holding
/// Cargo's dependency artefacts.
///
/// The gate requires a unit directory and a fingerprint store inside the
/// profile, not merely the profile directory, so a fixture that created only
/// the latter would make the first gate refuse for a reason other than the
/// eviction under test.
///
/// Every unit is written as rustc writes one — a `.d` record beside the output
/// it describes — because that pairing is what the discovery walk follows. A
/// bare placeholder file in `deps` would leave the profile with no units, and
/// the gate would refuse the eviction for the wrong reason.
///
/// One unit per declared package, not one unit in total. The preparation holds
/// its declaration to the record: a package named in `owned_packages` that
/// contributed no artefact is refused, because a stamp that named it would
/// vouch for units this preparation never built and their later eviction would
/// pass. A fixture that wrote a single unowned `unit` would therefore be
/// refused before any eviction ran. Writing one unit per declared package keeps
/// the fixture truthful about what it built, so the only reason a case can
/// refuse is the eviction it performs.
///
/// Every write goes through a [`Dir`] capability opened on the target
/// directory, with paths relative to it. The target is named by the child
/// environment, which a test controls, and the later evictions remove it
/// wholesale; confining the fixture's I/O to a handle on the intended directory
/// means a mistake in that path creates nothing outside it. `Dir` also refuses
/// a path that is absolute or that climbs out with `..`, so the confinement is
/// enforced rather than merely intended.
fn populate_profile(
    env: &process::ChildEnv,
    preparation: &nested_cargo::Preparation,
) -> std::io::Result<()> {
    let target = process::child_target_directory(env);
    let profile = format!("{}/deps", nested_cargo::DEFAULT_PROFILE);
    let fingerprints = format!("{}/.fingerprint", nested_cargo::DEFAULT_PROFILE);
    let root = cap_std::fs::Dir::open_ambient_dir(target, ambient_authority())?;
    root.create_dir_all(&profile)?;
    root.create_dir_all(&fingerprints)?;
    // A family that declares nothing claims its whole profile and needs only
    // the structural layer satisfied, so one unowned unit is exact for it.
    let packages: Vec<&str> = preparation
        .owned_packages
        .iter()
        .map(String::as_str)
        .collect();
    let packages = if packages.is_empty() {
        vec!["unit"]
    } else {
        packages
    };
    for package in packages {
        // Cargo hyphenates a package in its record directory and uses the same
        // stem in `deps`, which is the spelling `name_is_owned` matches.
        let stem = format!("{package}-0123456789abcdef");
        root.write(format!("{profile}/{stem}.d"), b"dep-info")?;
        root.write(format!("{profile}/lib{stem}.rlib"), b"artefact")?;
        root.create_dir_all(format!("{fingerprints}/{stem}"))?;
        root.write(
            format!("{fingerprints}/{stem}/record"),
            b"fingerprint record",
        )?;
    }
    Ok(())
}

/// A prepared family whose stamp the gate accepted.
///
/// Handed to the eviction cases, which differ only in what they then remove.
/// The temporary directory is a field rather than a return-value tail so the
/// fixtures owning it cannot be dropped while a case is still using the tree.
struct PreparedFamily {
    /// Owns the external target directory; dropped with this value.
    _external: tempfile::TempDir,
    /// The environment the preparation and the gate both saw.
    env: process::ChildEnv,
    /// The family description its stamp was written for.
    preparation: nested_cargo::Preparation,
    /// The profile directory the stamp vouches for.
    profile: std::path::PathBuf,
}

/// Prepare a family over a populated profile and require the stamp to be accepted.
///
/// The three eviction cases share the same four steps — an external target
/// directory, a family described against it, a populated profile, and a stamp
/// written and accepted — and then differ only in what they remove. Each used
/// to repeat all four, so a change to the preparation contract had to be made
/// three times and could be made correctly in two.
///
/// It is fallible because every step in it is: a temporary directory, the child
/// environment, the preparation description and the profile writes can each
/// fail. Returning the first failure lets the test body report why the fixture
/// could not be built instead of panicking inside it.
///
/// # Errors
///
/// Returns the first failure from creating the directories, resolving the
/// environment, describing the family, populating the profile, or writing and
/// accepting the stamp.
#[fixture]
fn prepared_family() -> std::io::Result<PreparedFamily> {
    let external = tempfile::tempdir()?;
    let env = child_env(external.path())?;
    let preparation = fixture_preparation(&env, REBUILD)?;
    populate_profile(&env, &preparation)?;
    let profile = process::child_target_directory(&env).join(nested_cargo::DEFAULT_PROFILE);

    nested_cargo::prepare_now(&preparation, || {
        Ok(nested_cargo::PreparationSummary::default())
    })?;
    // Whether a freshly-written stamp is accepted is a precondition of every
    // eviction case, not the behaviour any of them measures.
    nested_cargo::ensure_prepared_with(&preparation, true, || {
        panic!("a just-prepared family must satisfy the gate")
    })?;
    Ok(PreparedFamily {
        _external: external,
        env,
        preparation,
        profile,
    })
}

/// Remove `path` wholesale through a capability on its parent.
///
/// The eviction cases delete a whole directory, and a `Dir` capability cannot
/// remove the directory it is open on, so the capability is opened one level up
/// and the target removed by name. `Dir` refuses an absolute path or one that
/// climbs out with `..`, so a mistake in a fixture path cannot delete anything
/// outside the temporary directory the case owns.
fn evict(path: &std::path::Path) -> std::io::Result<()> {
    let description = || {
        std::io::Error::other(format!(
            "{} cannot be evicted through its parent",
            path.display()
        ))
    };
    let parent = path.parent().ok_or_else(description)?;
    let name = path.file_name().ok_or_else(description)?;
    let name = name.to_str().ok_or_else(description)?;
    cap_std::fs::Dir::open_ambient_dir(parent, ambient_authority())?.remove_dir_all(name)
}

/// A scratch copy of the fixture, standing in for `fixtures::scratch_root`.
fn scratch_fixture() -> std::io::Result<tempfile::TempDir> {
    let scratch = tempfile::tempdir()?;
    let root = cap_std::fs::Dir::open_ambient_dir(scratch.path(), ambient_authority())?;
    root.create_dir("fixture")?;
    root.write("fixture/Cargo.toml", b"[package]\nname = \"f\"\n")?;
    Ok(scratch)
}

#[test]
fn an_inherited_target_directory_moves_the_stamp_with_the_artefacts() {
    let external = tempfile::tempdir().expect("external target dir");
    let env = child_env(external.path()).expect("the fixture environment should resolve");
    assert_eq!(
        process::child_target_directory(&env),
        external.path(),
        "an inherited CARGO_TARGET_DIR must win over the workspace default"
    );

    let preparation = fixture_preparation(&env, REBUILD).expect("preparation description");

    assert!(
        preparation.directory.starts_with(external.path()),
        "the stamp must live inside the resolved target directory, not the scratch root: {}",
        preparation.directory.display()
    );
    assert_ne!(
        preparation.directory,
        fixtures::scratch_root(),
        "a stamp outside the external target would survive its eviction"
    );
}

#[test]
fn without_an_inherited_target_the_workspace_target_is_used() {
    let env =
        process::child_env_from_vars(Vec::new()).expect("the fixture environment should resolve");
    assert_eq!(
        process::child_target_directory(&env),
        fixtures::shared_target_dir()
    );
}

#[test]
fn a_configuration_only_change_invalidates_the_preparation() {
    // Cargo discovers configuration by walking up from the nested build's
    // working directory, so the committed workspace `.cargo/config.toml`
    // applies to both fixtures here. It sets the development-profile options
    // and the Linux linker flags: editing either rebuilds dependency artefacts
    // without touching a fixture source, so a stamp that ignored it would
    // survive the edit and the timed assertion would compile the graph.
    let workspace = super::workspace_root();
    let workspace_config = workspace.join(".cargo/config.toml");
    assert!(
        workspace_config.is_file(),
        "this regression is about the configuration this repository actually commits; {} is \
         missing",
        workspace_config.display()
    );

    let roots = workspace_inputs().expect("workspace inputs");
    assert!(
        roots.contains(&workspace_config),
        "the configuration must be among the family's own inputs: {roots:?}"
    );

    let with = nested_cargo::input_hash(&roots).expect("hash with the configuration");
    let without: Vec<std::path::PathBuf> = roots
        .iter()
        .filter(|root| *root != &workspace_config)
        .cloned()
        .collect();
    let without = nested_cargo::input_hash(&without).expect("hash without the configuration");
    assert_ne!(
        with, without,
        "a configuration-only change must invalidate the prepared artefacts"
    );
}

#[rstest::rstest]
fn evicting_the_artefacts_invalidates_the_preparation(
    #[from(prepared_family)] prepared: std::io::Result<PreparedFamily>,
) {
    // Prepare a family whose artefacts live outside the scratch tree, then
    // evict exactly that directory while leaving the scratch fixture alone.
    // The gate must refuse: the artefacts the stamp vouched for are gone,
    // and a timed assertion that trusted the stamp would rebuild the
    // dependency graph inside its own deadline.
    //
    // The fixture populated the profile before writing the stamp, so a
    // refusal at the end cannot come from artefacts that never existed.
    let prepared = prepared.expect("the prepared family fixture");
    let preparation = &prepared.preparation;
    let scratch = scratch_fixture().expect("scratch fixture");
    assert!(scratch.path().is_dir(), "the scratch fixture must survive");
    // Guard before evicting: a regression that stamped the shared scratch
    // root would otherwise leave a stamp in a directory this test does not
    // own.
    assert!(
        preparation
            .directory
            .starts_with(process::child_target_directory(&prepared.env)),
        "the stamp must live inside the resolved target directory: {}",
        preparation.directory.display()
    );

    evict(process::child_target_directory(&prepared.env)).expect("evict the artefact directory");

    let error = nested_cargo::ensure_prepared_with(preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("the evicted artefacts must fail the gate");
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert!(
        scratch.path().is_dir(),
        "the scratch fixture must be untouched"
    );
}

#[test]
fn a_relative_target_directory_stamps_where_the_child_builds() {
    // The nested build runs in the scratch fixture directory, not in this
    // process's directory, so Cargo would resolve a relative
    // `CARGO_TARGET_DIR` to a different place than the stamp did. The
    // harness must resolve it once, before storing it, so both name the
    // same tree and evicting that tree also evicts the stamp.
    let env = child_env(std::path::Path::new("relative-cache"))
        .expect("the fixture environment should resolve");
    let resolved = process::child_target_directory(&env);
    assert!(
        resolved.is_absolute(),
        "the stored target directory must be absolute: {}",
        resolved.display()
    );
    assert!(
        resolved.ends_with("relative-cache"),
        "the inherited value must be preserved as the final component: {}",
        resolved.display()
    );

    let preparation = fixture_preparation(&env, REBUILD).expect("preparation description");
    assert!(
        preparation.directory.starts_with(resolved),
        "the stamp must be under the resolved directory: {}",
        preparation.directory.display()
    );
}

#[rstest::rstest]
fn evicting_only_the_profile_invalidates_the_preparation(
    #[from(prepared_family)] prepared: std::io::Result<PreparedFamily>,
) {
    // `clean_fixture_units` runs `cargo clean -p <fixture>`, and a plain
    // `rm -rf <target>/debug` does the same thing more broadly. Both leave
    // the family's scratch root and its stamp untouched, because the
    // profile directory is a sibling of it. The gate must still refuse:
    // the compiled units the timed assertion reuses are gone.
    let prepared = prepared.expect("the prepared family fixture");
    let preparation = &prepared.preparation;

    evict(&prepared.profile).expect("evict only the profile");
    assert!(
        preparation.directory.is_dir(),
        "the family's scratch root and its stamp must survive: that is why the stamp alone cannot \
         answer this question"
    );

    let error = nested_cargo::ensure_prepared_with(preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("an evicted profile must fail the gate");
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
}

#[rstest::rstest]
fn evicting_the_compiled_units_invalidates_the_preparation(
    #[from(prepared_family)] prepared: std::io::Result<PreparedFamily>,
) {
    // The profile directory can survive while the work inside it does not.
    // This is the partial eviction a gate that only checked `is_dir()` would
    // accept: `<target>/debug` still exists, so does the family's stamp, and
    // Cargo must still rebuild every dependency unit.
    let prepared = prepared.expect("the prepared family fixture");
    let preparation = &prepared.preparation;

    // Remove exactly one of the two directories Cargo needs, then require the
    // gate to refuse. Removing both would also exercise it, but only this
    // shape proves that either one alone is enough to invalidate a stamp.
    evict(&prepared.profile.join("deps")).expect("evict the compiled units");
    assert!(
        prepared.profile.is_dir(),
        "the profile directory must survive"
    );
    assert!(
        preparation.directory.is_dir(),
        "the family's stamp must survive"
    );

    let error = nested_cargo::ensure_prepared_with(preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("an evicted dependency graph must fail the gate");
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    let message = error.to_string();
    assert!(
        message.contains("artefacts it vouched for are gone"),
        "the message must name the evicted artefacts: {message}"
    );
}
