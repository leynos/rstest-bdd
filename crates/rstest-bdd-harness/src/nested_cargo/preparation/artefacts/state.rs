//! What each named artefact directory turned out to hold.
//!
//! Reading the directories is separate from deciding what their state means.
//! The decision needs every directory's answer at once — a preparation is
//! refused when *none* of them holds a usable tree — so the two are split here
//! and the inspection is done once, up front.
//!
//! The distinction this module exists to preserve is between a profile that
//! holds nothing and one that has lost what it held. Cargo writes both shapes:
//! a build selected onto a target triple leaves the host profile with only lock
//! files, `build/`, `examples/` and `incremental/` when nothing in the graph
//! needs a host unit, and that is a successful build. Refusing it outright
//! refused every target-selected preparation of a dependency-free crate. What
//! separates the two states is not the directory's own shape but what the stamp
//! says the preparation left there, which is why this module only reports and
//! [`super::evicted`] decides.

use std::{fs, io, path::Path};

use super::super::layout;

/// What one named artefact directory holds, as far as the gate may rely on.
#[derive(Debug)]
pub(super) enum State {
    /// A stamp was found and records no artefacts for this directory.
    Unvouched,
    /// The directory is there, holding this layout.
    Present(layout::Layout),
    /// The directory is not there as a directory, described by the reason.
    Absent(&'static str),
}

impl State {
    /// Whether a prepared tree is here at all.
    ///
    /// False for an absent directory and for one that holds only half of what
    /// Cargo reads, which are the states a `cargo clean` leaves.
    pub(super) fn is_usable(&self) -> bool {
        matches!(self, Self::Present(layout) if layout.is_usable())
    }

    /// Why this directory is not there to be inspected, when it is not.
    ///
    /// Both answers refuse the preparation however healthy a sibling profile
    /// is. An unvouched directory belongs to some other preparation whatever it
    /// holds, and an absent one is a directory the build was asked to write and
    /// did not.
    pub(super) fn absence_reason(&self) -> Option<&'static str> {
        match *self {
            Self::Unvouched => Some("the stamp records no artefacts for this profile"),
            Self::Absent(reason) => Some(reason),
            Self::Present(_) => None,
        }
    }

    /// Why this directory holds no tree Cargo could reuse, when it holds none.
    ///
    /// A *waivable* refusal: see [`super::evicted`]. A build selected onto a
    /// target triple leaves the host profile exactly here, so this state alone
    /// does not prove an eviction — only the stamp, or the absence of any
    /// populated sibling, can decide that.
    ///
    /// `None` for a usable directory and for an absent one, which is refused
    /// through [`State::absence_reason`] instead.
    pub(super) fn empty_layout_reason(&self) -> Option<&'static str> {
        match *self {
            Self::Present(ref layout) if layout.units.is_empty() => Some("holds no compiled units"),
            Self::Present(ref layout) if layout.fingerprints.is_empty() => {
                Some("holds no fingerprint records")
            }
            Self::Present(_) | Self::Unvouched | Self::Absent(_) => None,
        }
    }

    /// This directory's layout, when it has one to inspect.
    pub(super) fn layout(&self) -> Option<&layout::Layout> {
        match self {
            Self::Present(layout) => Some(layout),
            Self::Unvouched | Self::Absent(_) => None,
        }
    }
}

/// Inspect `directory`, unless `vouched` is false.
///
/// `vouched` comes first so an unvouched directory is reported without a
/// `stat`: its contents belong to some other preparation, whatever they happen
/// to be, and reading them would only invite a conclusion about them.
///
/// # Errors
///
/// Returns the error when the directory exists but cannot be inspected, and the
/// read error from the layout walk. Both refuse the preparation; neither is
/// folded into an eviction, because a tree that cannot be read is not a tree
/// known to be gone.
pub(super) fn inspect(directory: &Path, vouched: bool) -> io::Result<State> {
    if !vouched {
        return Ok(State::Unvouched);
    }
    match fs::metadata(directory) {
        Ok(metadata) if metadata.is_dir() => Ok(State::Present(layout::discover(directory)?)),
        // Absent, or present as something that is not a directory: both are the
        // ordinary eviction, and neither has a layout to discover.
        Ok(_) => Ok(State::Absent("not a directory")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(State::Absent("absent")),
        // A permission or I/O fault must name itself. Folding it into an
        // eviction would report a tree that cannot be inspected as one that is
        // known to be empty, which is a diagnosis the reader cannot act on.
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;
