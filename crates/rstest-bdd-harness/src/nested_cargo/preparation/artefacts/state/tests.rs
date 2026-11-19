//! What the inspection reports, and what the gate then makes of it.
//!
//! The distinction these cases protect is between a profile that holds nothing
//! and one that has lost what it held. Both look the same from inside the
//! directory; only what the stamp says the preparation left there tells them
//! apart, so each case states the state and the stamp section together.

use std::{fs, io};

use super::{State, inspect};
use crate::nested_cargo::{
    Directories,
    Preparation,
    ProfileDestination,
    Sources,
    env_from_vars,
    preparation::artefacts::{evicted, record},
};

/// Write a profile holding one compiled unit and its fingerprint record.
fn populated(profile: &std::path::Path) -> io::Result<()> {
    let deps = profile.join("deps");
    fs::create_dir_all(&deps)?;
    fs::create_dir_all(profile.join(".fingerprint/unit-hash"))?;
    fs::write(profile.join(".fingerprint/unit-hash/unit"), b"record")?;
    fs::write(deps.join("unit.d"), b"dep-info")?;
    fs::write(deps.join("libunit.rlib"), b"unit")?;
    Ok(())
}

/// A directory holding only what Cargo leaves when nothing needs a host unit.
///
/// Measured on this host, not assumed: a dependency-free library built with
/// `cargo build --target x86_64-unknown-linux-gnu` left exactly these entries
/// under `target/debug`, with no dep-info record and no fingerprint store
/// anywhere beneath it.
fn host_only(profile: &std::path::Path) -> io::Result<()> {
    for entry in ["build", "examples", "incremental"] {
        fs::create_dir_all(profile.join(entry))?;
    }
    fs::write(profile.join(".cargo-lock"), b"")?;
    Ok(())
}

/// Describe a family that vouches for `profiles`, all beneath one target.
fn preparation_for(profiles: &[&std::path::Path]) -> io::Result<Preparation> {
    let root = profiles
        .first()
        .ok_or_else(|| io::Error::other("a case must name at least one profile"))?;
    let target = root
        .parent()
        .ok_or_else(|| io::Error::other("a profile must have a target directory"))?;
    let environment = env_from_vars(Vec::new(), Some(target), ProfileDestination::Caller)?;
    let mut preparation = Preparation::new(
        Sources::new("unit", "v", "hash"),
        Directories::new(root.to_path_buf(), root.to_path_buf()),
        environment,
    )?;
    preparation.artefacts = profiles.iter().map(|path| path.to_path_buf()).collect();
    Ok(preparation)
}

/// The description of a single eviction, for a case expecting exactly one.
///
/// Taking the one element out of the list rather than indexing it keeps the
/// "exactly one" expectation in the type: a case that accidentally produced two
/// fails here, naming both, instead of quietly reporting on the first.
fn only(evictions: &[crate::nested_cargo::preparation::artefacts::Eviction]) -> String {
    match evictions {
        [eviction] => eviction.describe(),
        _ => panic!("exactly one profile must be refused, got {evictions:?}"),
    }
}

#[test]
fn a_populated_profile_is_present_and_usable() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let profile = directory.path().join("debug");
    populated(&profile)?;
    let state = inspect(&profile, true)?;
    assert!(state.is_usable(), "a built profile is usable: {state:?}");
    assert_eq!(state.absence_reason(), None);
    assert_eq!(state.empty_layout_reason(), None);
    Ok(())
}

#[test]
fn an_unvouched_directory_is_refused_before_it_is_read() -> io::Result<()> {
    // The directory here is populated, and deliberately so: an unvouched one is
    // refused whatever it holds, because its contents belong to some other
    // preparation and reading them would only invite a conclusion about them.
    let directory = tempfile::tempdir()?;
    let profile = directory.path().join("debug");
    populated(&profile)?;
    let state = inspect(&profile, false)?;
    assert!(matches!(state, State::Unvouched), "{state:?}");
    assert!(
        state.absence_reason().is_some(),
        "an unvouched profile is refused"
    );
    assert_eq!(
        state.empty_layout_reason(),
        None,
        "it is not refused for its shape, which was never read"
    );
    Ok(())
}

#[test]
fn an_absent_profile_names_its_absence() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let profile = directory.path().join("debug");
    let state = inspect(&profile, true)?;
    assert!(matches!(state, State::Absent("absent")), "{state:?}");
    assert_eq!(state.absence_reason(), Some("absent"));
    Ok(())
}

#[test]
fn a_file_where_a_profile_belongs_is_not_a_directory() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let profile = directory.path().join("debug");
    fs::write(&profile, b"not a directory")?;
    let state = inspect(&profile, true)?;
    assert!(
        matches!(state, State::Absent("not a directory")),
        "{state:?}"
    );
    Ok(())
}

/// The regression for a dependency-free crate built with a target selection.
///
/// `cargo build --target <triple>` on a crate with no build script and no
/// proc-macro dependency leaves the host profile holding only lock files,
/// `build/`, `examples/` and `incremental/`. The gate refused that with "holds
/// no compiled units" and failed a preparation whose command had just exited
/// zero, so every such family was unusable.
///
/// The stamp records nothing for that profile — there was nothing to record —
/// and the sibling under the triple holds the units, which is exactly the state
/// that must now be admitted.
#[test]
fn a_target_selected_preparation_with_an_empty_host_profile_is_admitted() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let target = directory.path().join("target");
    let host = target.join("debug");
    let selected = target.join("x86_64-unknown-linux-gnu/debug");
    host_only(&host)?;
    populated(&selected)?;

    let preparation = preparation_for(&[&host, &selected])?;
    let recorded = record(&preparation)?;
    assert_eq!(
        recorded.first().map(Vec::len),
        Some(0),
        "the host profile holds nothing, so its section records nothing: {recorded:?}"
    );
    assert!(
        recorded.get(1).is_some_and(|section| !section.is_empty()),
        "the selected profile's section names its units: {recorded:?}"
    );
    assert!(
        evicted(&preparation, None)?.is_empty(),
        "a target-selected build is not evicted merely because its host profile is empty"
    );
    Ok(())
}

/// The other side of the same rule: an empty profile beside no populated one.
///
/// A build that produced nothing at all must still refuse, or the waiver would
/// admit a preparation whose command left no tree anywhere.
#[test]
fn an_empty_profile_beside_no_populated_one_is_refused() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let target = directory.path().join("target");
    let host = target.join("debug");
    let selected = target.join("x86_64-unknown-linux-gnu/debug");
    host_only(&host)?;
    host_only(&selected)?;

    let preparation = preparation_for(&[&host, &selected])?;
    let evictions = evicted(&preparation, None)?;
    assert_eq!(
        evictions.len(),
        2,
        "both empty profiles must be reported: {evictions:?}"
    );
    Ok(())
}

/// An empty profile that is the *only* one a family names is refused.
///
/// [`Preparation::for_fixture`] names exactly one directory, so the waiver —
/// which needs a populated sibling — must not reach it. `mkdir -p <root>/debug`
/// is not a build, and this is the case that says so.
#[test]
fn a_sole_empty_profile_is_refused() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let profile = directory.path().join("debug");
    host_only(&profile)?;
    let preparation = preparation_for(&[&profile])?;
    let evictions = evicted(&preparation, None)?;
    let refusal = only(&evictions);
    assert!(
        refusal.contains("holds no compiled units"),
        "the reason must name what the profile lacks: {refusal}"
    );
    Ok(())
}

/// A host profile that held nothing and later gained a *half* still refuses.
///
/// The waiver covers a directory holding neither half, not one holding only
/// some of what Cargo reads. This is a partial clean beside a populated
/// sibling, which must never be admitted just because the rest of the family
/// looks healthy.
#[test]
fn a_half_empty_profile_beside_a_populated_one_is_refused() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let target = directory.path().join("target");
    let host = target.join("debug");
    let selected = target.join("x86_64-unknown-linux-gnu/debug");
    populated(&host)?;
    fs::remove_dir_all(host.join(".fingerprint"))?;
    populated(&selected)?;

    let preparation = preparation_for(&[&host, &selected])?;
    let evictions = evicted(&preparation, None)?;
    let refusal = only(&evictions);
    assert!(
        refusal.contains("holds no fingerprint records"),
        "the reason must name the half that went: {refusal}"
    );
    Ok(())
}

/// An emptied profile the stamp *had* recorded refuses, even beside a sibling.
///
/// This is the case the waiver must not swallow. The host profile was populated
/// when the preparation ran — so its stamp section names units — and only a
/// later clean left it empty. The stamp, not the directory's shape, is what
/// distinguishes this from a build that never needed host units, and the exact
/// layer is where that distinction is decided.
#[test]
fn a_host_profile_the_stamp_had_recorded_refuses_once_emptied() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let target = directory.path().join("target");
    let host = target.join("debug");
    let selected = target.join("x86_64-unknown-linux-gnu/debug");
    populated(&host)?;
    populated(&selected)?;

    let preparation = preparation_for(&[&host, &selected])?;
    let recorded = record(&preparation)?;
    assert!(
        recorded.first().is_some_and(|section| !section.is_empty()),
        "the fixture must start with a recorded host profile: {recorded:?}"
    );
    assert!(
        evicted(
            &preparation,
            Some(&crate::nested_cargo::preparation::stamp::Stamp {
                recorded: recorded.clone()
            })
        )?
        .is_empty(),
        "the fixture must be admitted before the eviction, or the case proves nothing"
    );

    fs::remove_dir_all(&host)?;
    fs::create_dir_all(&host)?;
    let stamp = crate::nested_cargo::preparation::stamp::Stamp { recorded };
    let evictions = evicted(&preparation, Some(&stamp))?;
    let [eviction] = evictions.as_slice() else {
        panic!("the emptied profile was vouched for by name and must refuse: {evictions:?}");
    };
    assert!(
        !eviction.missing.is_empty(),
        "the recorded names that went must be reported: {:?}",
        eviction.missing
    );
    Ok(())
}
