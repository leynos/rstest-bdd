//! Recording the artefact names a preparation left behind.
//!
//! The exact half of [`artefacts`](super). A stamp records the outputs the
//! preparation produced, beside the dep-info records Cargo uses to decide a
//! unit is current, so the gate can later require every one of them to still
//! exist. Nothing here inspects the tree to judge it: this module only says what
//! was there, and [`artefacts`](super) decides what its absence means.
//!
//! Two kinds of name, both required. A compiled output whose file was removed
//! is one Cargo recompiles, and a unit whose fingerprint record was removed is
//! also one Cargo recompiles even though every output it produced is still
//! present — the record is what says those outputs are current. Neither is
//! recoverable from the other, which is why both are recorded.

use std::{
    io,
    path::{Path, PathBuf},
};

use super::{
    super::{
        Preparation,
        layout::{self, Layout},
    },
    ownership,
};

/// The name a recorded output is filed under.
///
/// The unit directory is named relative to the profile and joined with `/` on
/// every platform, so the same tree produces the same stamp text on Windows and
/// on Unix, and a stamp written by one host is readable by another. The
/// directory has to be part of the name: two units in different directories can
/// share a stem, and a record that named only the stem would let one unit's
/// output witness the other's.
pub(super) fn recorded_name(profile: &Path, directory: &Path, output: &str) -> String {
    let relative: Vec<String> = directory.strip_prefix(profile).map_or_else(
        |_| vec![directory.to_string_lossy().into_owned()],
        |relative| {
            relative
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect()
        },
    );
    if relative.is_empty() {
        return output.to_owned();
    }
    format!("{}/{output}", relative.join("/"))
}

/// The recorded names a layout vouches for, sorted.
///
/// Two kinds of name, both required, for reasons that are independent. A
/// compiled output whose file was removed is one Cargo recompiles. A unit whose
/// fingerprint record was removed is *also* one Cargo recompiles, even though
/// every output it produced is still there, because the record is what says
/// those outputs are current. Recording only outputs therefore admits a tree
/// the next build silently rewrites — and the store itself still looks
/// populated, because emptying one record's directory leaves the others.
///
/// The records come from the store directories the discovery walk found, and
/// each is filed under the unit it belongs to, so the name is qualified by the
/// store and by the unit's own directory exactly as an output is qualified by
/// its unit directory.
///
/// Only names belonging to `owned` are kept, so a family that shares a profile
/// does not vouch for a sibling's units; see the `ownership` module. An empty
/// `owned` keeps every name.
///
/// `owned` must name every package the preparation actually built, dependencies
/// included: declaring only the fixture filters its workspace dependencies out
/// of both lists, so an evicted dependency compares equal to itself and passes.
/// The admission is silent — it names no package — so the caller, the only
/// place the dependency list is known, has to declare it.
///
/// # Errors
///
/// Returns the read error when a unit directory or record directory cannot be
/// listed.
pub(super) fn recorded_names(
    profile: &Path,
    layout: &Layout,
    owned: &[String],
) -> io::Result<Vec<String>> {
    let mut names = unit_names(profile, &layout.units)?;
    names.append(&mut record_names(profile, &layout.fingerprints)?);
    // A family speaks only for what it built. See the `ownership` module for
    // why the distinction cannot be recovered from the tree itself.
    names.retain(|name| ownership::name_is_owned(name, owned));
    names.sort();
    names.dedup();
    Ok(names)
}

/// The compiled outputs each unit directory holds, qualified by that directory.
///
/// # Errors
///
/// Returns the read error when a unit directory cannot be listed.
pub(super) fn unit_names(profile: &Path, units: &[PathBuf]) -> io::Result<Vec<String>> {
    let mut names = Vec::new();
    for directory in units {
        for output in layout::outputs_in(directory)? {
            names.push(recorded_name(profile, directory, &output));
        }
    }
    Ok(names)
}

/// The fingerprint records in each store, qualified by store and unit.
///
/// A record is filed under the unit it belongs to, so the name carries both
/// components' worth of path; see [`recorded_names`] for why the record is
/// required at all.
///
/// # Errors
///
/// Returns the read error when a store or a record directory cannot be listed.
pub(super) fn record_names(profile: &Path, stores: &[PathBuf]) -> io::Result<Vec<String>> {
    let mut names = Vec::new();
    for store in stores {
        for unit in layout::record_directories(store)? {
            for file in layout::entries_of(&unit)? {
                names.push(recorded_name(profile, &unit, &file));
            }
        }
    }
    Ok(names)
}

/// Record the artefact names a freshly prepared family vouches for.
///
/// Called after the preparation command succeeded, so the names are what that
/// command actually left rather than what it was expected to leave. One list
/// per [`Preparation::artefacts`] entry, in that order.
///
/// A profile that does not exist records nothing rather than failing: the
/// caller runs [`super::verify_structural`] first, which has already refused that
/// state with the diagnosis worth reporting. Reaching here with one absent
/// would mean an eviction between the two calls, and an empty record there is
/// still refused by [`super::verify_recorded`]'s structural half.
///
/// A name that is not valid Unicode is recorded lossily: every real unit name is
/// ASCII, and refusing on one would fail a preparation for a reason it cannot
/// name.
///
/// # Errors
///
/// Returns the read error when a profile that exists cannot be inspected.
pub(in crate::nested_cargo::preparation) fn record(
    preparation: &Preparation,
) -> io::Result<Vec<Vec<String>>> {
    preparation
        .artefacts
        .iter()
        .map(|profile| {
            recorded_names(
                profile,
                &layout::discover(profile)?,
                &preparation.owned_packages,
            )
        })
        .collect()
}
