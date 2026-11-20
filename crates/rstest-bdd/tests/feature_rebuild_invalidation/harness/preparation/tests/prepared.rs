//! The shared fixture machinery for the stamp-location regressions.
//!
//! Named `prepared` rather than `fixtures` because the parent harness already
//! has a `fixtures` module; a second one under this path would shadow it in
//! every `use crate::harness::fixtures` in this tree, which is a confusion no
//! reader should have to resolve.
//!
//! The eviction cases differ only in what they remove, so everything they have
//! in common lives here: an external target directory, a family described
//! against it, a populated profile, and a stamp written and accepted. Keeping
//! that in one place means a change to the preparation contract is made once;
//! repeated per case it had to be made three times and could be made correctly
//! in two.
//!
//! Split from [`super`] so neither module exceeds the repository's per-file
//! budget. The cases that consume these helpers stay beside the assertions
//! they make about the gate; only the setup moved.

use std::ffi::OsString;

use cap_std::ambient_authority;
use rstest::fixture;
use rstest_bdd_harness::nested_cargo;

use super::super::{REBUILD, fixture_preparation};
use crate::harness::process;

/// The stamp file a preparation writes.
///
/// Named here from the same rule the library uses — `.` + the family name plus
/// `.rstest-bdd-prepared`, inside the artefact directory — because the library
/// keeps its own `stamp_path` helper private to the preparation module. A case
/// that asserts the *stamp* survived eviction rather than merely its parent
/// directory needs to name the file, and naming it by the documented rule is
/// what keeps the assertion honest about which artefact it is checking.
pub(super) fn stamp_file(preparation: &nested_cargo::Preparation) -> std::path::PathBuf {
    preparation
        .directory
        .join(format!(".{}.rstest-bdd-prepared", preparation.family))
}

/// Refuse a preparation whose stamp would land outside `root`.
///
/// The eviction cases own a temporary directory and assert that evicting
/// something inside it invalidates the stamp. Both halves of that claim depend
/// on the stamp being *inside* it: a stamp written to the shared scratch root
/// would survive every eviction the cases perform, so they would report a
/// passing gate for a reason unrelated to what they meant to test.
///
/// Checking here, before anything is written, is what keeps a regression in
/// target resolution from leaving a stamp behind in the shared tree. The
/// assertion in the consuming case still states the contract; this makes it a
/// precondition instead of a post-mortem.
///
/// # Errors
///
/// Returns an [`std::io::ErrorKind::InvalidInput`] error naming the resolved
/// directory when it does not lie beneath `root`.
pub(super) fn require_stamp_under(
    root: &std::path::Path,
    preparation: &nested_cargo::Preparation,
) -> std::io::Result<()> {
    if preparation.directory.starts_with(root) {
        return Ok(());
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!(
            "the stamp would be written to {}, outside the temporary directory {} this case owns; \
             an eviction inside that directory could not invalidate it",
            preparation.directory.display(),
            root.display()
        ),
    ))
}

/// A child environment whose artefacts go to `target_directory`.
pub(super) fn child_env(target_directory: &std::path::Path) -> std::io::Result<process::ChildEnv> {
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
pub(super) fn populate_profile(
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
pub(super) struct PreparedFamily {
    /// Owns the external target directory; dropped with this value.
    _external: tempfile::TempDir,
    /// The environment the preparation and the gate both saw.
    pub(super) env: process::ChildEnv,
    /// The family description its stamp was written for.
    pub(super) preparation: nested_cargo::Preparation,
    /// The profile directory the stamp vouches for.
    pub(super) profile: std::path::PathBuf,
}

/// Prepare a family over a populated profile and require the stamp to be accepted.
///
/// The eviction cases share the same four steps — an external target
/// directory, a family described against it, a populated profile, and a stamp
/// written and accepted — and then differ only in what they remove.
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
pub(super) fn prepared_family() -> std::io::Result<PreparedFamily> {
    let external = tempfile::tempdir()?;
    let env = child_env(external.path())?;
    let preparation = fixture_preparation(&env, REBUILD)?;
    // Checked before the fixture writes anything, not only in the cases that
    // consume it. If target resolution ever regressed to the shared scratch
    // root, `prepare_now` would write a stamp there — outside the temporary
    // directory this case owns — and a later assertion would report it after
    // the damage rather than instead of it. Every eviction case gets this
    // guard by using the fixture.
    require_stamp_under(external.path(), &preparation)?;
    populate_profile(&env, &preparation)?;
    let profile = process::child_target_directory(&env).join(nested_cargo::DEFAULT_PROFILE);

    nested_cargo::prepare_now(&preparation, || {
        Ok(nested_cargo::PreparationSummary::default())
    })?;
    // Whether a freshly-written stamp is accepted is a precondition of every
    // eviction case, not the behaviour any of them measures.
    nested_cargo::ensure_prepared_building(&preparation, true, || {
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
pub(super) fn evict(path: &std::path::Path) -> std::io::Result<()> {
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
pub(super) fn scratch_fixture() -> std::io::Result<tempfile::TempDir> {
    let scratch = tempfile::tempdir()?;
    let root = cap_std::fs::Dir::open_ambient_dir(scratch.path(), ambient_authority())?;
    root.create_dir("fixture")?;
    root.write("fixture/Cargo.toml", b"[package]\nname = \"f\"\n")?;
    Ok(scratch)
}
