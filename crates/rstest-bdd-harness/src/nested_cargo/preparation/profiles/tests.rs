//! The profiles a nested build writes when a target is selected.
//!
//! Cargo writes two profiles in that mode, and only one of them holds the
//! fixture's own units. `<target>/<triple>/debug` receives everything built for
//! the selected target, while `<target>/debug` receives the host-side units the
//! same build needs — its build scripts and proc macros. The difference is
//! invisible from `cargo metadata`, which reports the target directory
//! unchanged, so a descriptor derived from that alone names a profile that
//! holds the wrong half of the tree.
//!
//! Measured on this host, not assumed: a leaf crate built with
//! `--target x86_64-unknown-linux-gnu` left `libleaf.d` and `libleaf.rlib`
//! under `<target>/x86_64-unknown-linux-gnu/debug` and only lock files,
//! `build/`, `examples/` and `incremental/` under `<target>/debug`. A crate
//! with a proc-macro dependency left its host units under
//! `<target>/debug/build/...` and its own under the triple.

use std::path::PathBuf;

use super::profiles;
use crate::nested_cargo::{DEFAULT_PROFILE, ProfileDestination, env_from_vars};

/// The triple every case selects, unless the case is about the selection's
/// shape rather than its value.
const TRIPLE: &str = "x86_64-unknown-linux-gnu";

/// A scratch target directory and the profiles it yields.
///
/// `selection` is what `CARGO_BUILD_TARGET` carries, so a case can name a
/// triple, a path to a specification file, an empty value, or nothing at all.
/// The environment holds that variable and nothing else, so a case cannot pass
/// because of an inherited value it did not set.
fn profiles_for(
    selection: Option<&str>,
) -> std::io::Result<(tempfile::TempDir, PathBuf, Vec<PathBuf>)> {
    let directory = tempfile::tempdir()?;
    let target = directory.path().join("target");
    let mut vars: Vec<(std::ffi::OsString, std::ffi::OsString)> = Vec::new();
    if let Some(selection) = selection {
        vars.push(("CARGO_BUILD_TARGET".into(), selection.into()));
    }
    let environment = env_from_vars(vars, Some(&target), ProfileDestination::Caller)?;
    let named = profiles(&target, &environment);
    Ok((directory, target, named))
}

#[test]
fn a_host_only_selection_names_one_profile() -> std::io::Result<()> {
    // The configuration every family here uses today, and the one that must
    // keep working unchanged: no target selected, so Cargo writes only
    // `<target>/debug`, and naming a second profile would refuse a prepared
    // family for a directory that is never created.
    let (_directory, target, artefacts) = profiles_for(None)?;
    assert_eq!(
        artefacts,
        vec![target.join(DEFAULT_PROFILE)],
        "one profile without a selection"
    );
    Ok(())
}

#[test]
fn a_target_selection_names_both_profiles() -> std::io::Result<()> {
    // The regression: the fixture's units go beneath `<target>/<triple>/debug`
    // and the host's beneath `<target>/debug`. Naming only the second vouched
    // for a profile holding the host units, so evicting the directory the
    // fixture's own units were in left the stamp matching and moved the
    // compilation back into the timed assertion.
    let (_directory, target, artefacts) = profiles_for(Some(TRIPLE))?;
    assert_eq!(
        artefacts,
        vec![
            target.join(DEFAULT_PROFILE),
            target.join(TRIPLE).join(DEFAULT_PROFILE),
        ],
        "a selected target writes two profiles, and both must be vouched for"
    );
    Ok(())
}

#[test]
fn a_specification_file_selection_names_its_stem() -> std::io::Result<()> {
    // Cargo names the profile after the specification file's stem for a
    // path-valued target. A profile named after the whole path would be a
    // directory Cargo never writes, and the gate would refuse every prepared
    // family on such a host.
    let (_directory, target, artefacts) = profiles_for(Some("targets/my-spec.json"))?;
    assert_eq!(
        artefacts,
        vec![
            target.join(DEFAULT_PROFILE),
            target.join("my-spec").join(DEFAULT_PROFILE),
        ],
        "a specification file names its profile by its stem"
    );
    Ok(())
}

#[test]
fn a_dotted_builtin_target_keeps_its_whole_name() -> std::io::Result<()> {
    // `thumbv8m.main-none-eabi` is a built-in triple, and it carries a dot.
    // Taking its file stem — the obvious way to handle a specification file —
    // cuts it at the dot and names `target/thumbv8m/debug`, a directory Cargo
    // never writes. The gate would then refuse a prepared family on every host
    // using this target, which is why only a `.json` selection may be reduced
    // to a stem.
    let dotted = "thumbv8m.main-none-eabi";
    let (_directory, target, artefacts) = profiles_for(Some(dotted))?;
    assert_eq!(
        artefacts,
        vec![
            target.join(DEFAULT_PROFILE),
            target.join(dotted).join(DEFAULT_PROFILE),
        ],
        "a dotted built-in target names its own directory in full"
    );
    Ok(())
}

#[test]
fn a_padded_selection_names_the_unpadded_profile() -> std::io::Result<()> {
    // Cargo trims a padded `build.target` and selects the unpadded triple, so
    // the profile directory it writes carries no spaces. Deriving the path from
    // the raw value names a directory that does not exist, and the gate
    // refuses a tree that is in fact prepared.
    let (_directory, target, artefacts) = profiles_for(Some(&format!("  {TRIPLE}  ")))?;
    assert_eq!(
        artefacts,
        vec![
            target.join(DEFAULT_PROFILE),
            target.join(TRIPLE).join(DEFAULT_PROFILE),
        ],
        "surrounding whitespace must not reach the profile path"
    );
    Ok(())
}

#[test]
fn an_empty_selection_is_not_a_selection() -> std::io::Result<()> {
    // Cargo ignores an empty `CARGO_BUILD_TARGET` exactly as it ignores an
    // absent one, so naming a profile for it would refuse a tree that is
    // prepared. The value reaches the child unchanged, so the descriptor has to
    // make the same distinction Cargo does.
    let (_directory, target, artefacts) = profiles_for(Some(""))?;
    assert_eq!(
        artefacts,
        vec![target.join(DEFAULT_PROFILE)],
        "an empty selection writes one profile"
    );
    Ok(())
}

#[test]
fn a_selection_without_a_name_adds_no_profile() -> std::io::Result<()> {
    // A selection of `.` or `..` has no file stem, so it cannot name a
    // directory. Naming the parent of the target directory instead would vouch
    // for an unrelated tree, which is worse than refusing: the preparation then
    // fails on a missing profile and the message names what it looked for.
    for selection in [".", "..", "/"] {
        let (_directory, target, artefacts) = profiles_for(Some(selection))?;
        assert_eq!(
            artefacts,
            vec![target.join(DEFAULT_PROFILE)],
            "`{selection}` cannot name a profile, so only the host one is vouched for"
        );
    }
    Ok(())
}

#[test]
fn the_target_profile_is_reachable_from_the_artefact_list() -> std::io::Result<()> {
    // The CLI witness searches every profile the family names, so the one
    // holding the fixture's binaries has to be in that list. This states the
    // contract the witness relies on rather than the witness's own behaviour,
    // which its module tests cover.
    let (_directory, target, artefacts) = profiles_for(Some(TRIPLE))?;
    assert!(
        artefacts.contains(&target.join(TRIPLE).join(DEFAULT_PROFILE)),
        "the profile a selected build writes must be named: {artefacts:?}"
    );
    assert!(
        artefacts.iter().all(|path| path
            .parent()
            .is_some_and(|parent| !parent.as_os_str().is_empty())),
        "every named profile must be a directory with a parent: {artefacts:?}"
    );
    Ok(())
}
