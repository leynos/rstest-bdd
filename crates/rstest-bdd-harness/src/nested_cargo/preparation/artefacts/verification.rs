//! Refusing a preparation that did not leave what it claims to have left.
//!
//! These are the post-build half of the artefact check. [`super::evicted`]
//! answers what is wrong with a tree at any moment; these run *after* a build
//! and turn that answer into the preparation's error, so a stamp is never
//! written over a state the next timed assertion would have to rebuild.
//!
//! They live beside the judgement rather than in it so the parent module stays
//! inside the repository's per-file budget, and are re-exported from
//! [`super`] so no call site names this path.

use std::io;

use super::{Eviction, Preparation, evicted, ownership};
use crate::nested_cargo::preparation::stamp::Stamp;

/// Refuse a just-finished preparation whose profile is not structurally usable.
///
/// A preparation command can succeed — Cargo exits zero — while the artefacts
/// it was meant to produce are not there, for example when a concurrent `cargo
/// clean` ran between the build and this check. Writing a stamp over that
/// state would advertise a prepared family whose next timed assertion pays for
/// the rebuild, so the caller is told which directories are missing instead.
///
/// # Errors
///
/// Returns [`io::ErrorKind::NotFound`] naming every unusable profile, and the
/// read error when a profile cannot be inspected.
pub(in crate::nested_cargo::preparation) fn verify_structural(
    preparation: &Preparation,
) -> io::Result<()> {
    refuse_evictions(preparation, &evicted(preparation, None)?)
}

/// Refuse a just-finished preparation whose recorded artefacts have gone.
///
/// The second half of [`verify_structural`]: a unit's outputs can disappear
/// between being recorded and being stamped, which leaves every directory the
/// structural layer names in place.
///
/// # Errors
///
/// As [`verify_structural`].
pub(in crate::nested_cargo::preparation) fn verify_recorded(
    preparation: &Preparation,
    recorded: &[Vec<String>],
) -> io::Result<()> {
    let stamp = Stamp {
        recorded: recorded.to_vec(),
    };
    refuse_evictions(preparation, &evicted(preparation, Some(&stamp))?)
}

/// Refuse a preparation whose declared packages contributed no recorded name.
///
/// [`Preparation::owned_packages`] is a claim: "these packages are what my
/// preparation built, so my stamp speaks for their units". The ownership
/// filter turns that claim into a *restriction* — names outside it are dropped
/// from the record — but a restriction alone cannot detect a claim that is
/// wrong in the safe-looking direction. A package named there that the build
/// never actually produced contributes nothing to the record, so its units are
/// silently absent from the stamp; evicting them later then compares an empty
/// list with an empty list and passes the gate, which is precisely the failure
/// the ownership rule exists to prevent. The same happens for a name that is
/// misspelled or stale after a package rename.
///
/// Checking each declared package for at least one recorded name closes that
/// hole. It is deliberately run after [`record`] and before the stamp is
/// written: the record is what the command actually left, so a disagreement
/// between declaration and reality is visible there and nowhere else.
///
/// Skipped when `owned_packages` is empty, because an empty list means the
/// family claims its whole target directory and has declared nothing to check.
///
/// # Errors
///
/// Returns [`io::ErrorKind::InvalidInput`] naming every declared package that
/// contributed no recorded name.
pub(in crate::nested_cargo::preparation) fn verify_declared(
    preparation: &Preparation,
    recorded: &[Vec<String>],
) -> io::Result<()> {
    if preparation.owned_packages.is_empty() {
        return Ok(());
    }
    let names: Vec<&String> = recorded.iter().flatten().collect();
    let unbuilt: Vec<&str> = preparation
        .owned_packages
        .iter()
        .filter(|package| {
            !names
                .iter()
                .any(|name| ownership::name_is_owned(name, std::slice::from_ref(*package)))
        })
        .map(String::as_str)
        .collect();
    if unbuilt.is_empty() {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "preparation of family `{}` declares owned packages that contributed no recorded \
             artefact: {}. A stamp that named them would vouch for units this preparation never \
             built, so their eviction would pass the gate unnoticed",
            preparation.family,
            unbuilt.join(", ")
        ),
    ))
}

/// Turn a non-empty eviction list into the preparation failure.
fn refuse_evictions(preparation: &Preparation, evicted: &[Eviction]) -> io::Result<()> {
    if evicted.is_empty() {
        return Ok(());
    }
    let described: Vec<String> = evicted.iter().map(Eviction::describe).collect();
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "preparation of family `{}` finished but left no usable artefacts in {}",
            preparation.family,
            described.join(", ")
        ),
    ))
}
