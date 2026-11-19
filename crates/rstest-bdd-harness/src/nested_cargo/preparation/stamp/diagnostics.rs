//! The messages a refused or unreadable stamp produces.
//!
//! Both builders are pure: they turn facts the caller has already gathered
//! into text, and inspect nothing. Reading the artefact directories or writing
//! to standard error is the lifecycle caller's job, which is what keeps the
//! stamp module's file access in one place and its diagnostics testable
//! without a filesystem.

use std::{io, path::Path};

use super::super::Preparation;

/// The fail-fast error for a stamp that exists but cannot be read.
///
/// Under `nextest` this is a distinct condition from a missing stamp: the tree
/// is there, and the reader cannot say whether it matches. Reporting it as
/// "missing or stale" would send the reader looking for an evicted directory
/// that is present, so the message names the read failure and the remedy.
pub(in crate::nested_cargo::preparation) fn unreadable_stamp_error(
    stamp: &Path,
    error: &io::Error,
) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "the preparation stamp {} exists but cannot be read ({error}), so the prepared \
             artefacts cannot be vouched for; re-run `make prepare-nested-fixtures`",
            stamp.display()
        ),
    )
}

/// The fail-fast message for a timed test with no prepared artefacts.
///
/// `evicted` is what the caller found when it inspected the artefact
/// directories — one description per directory whose recorded artefacts have
/// gone. It arrives as a value rather than being computed here because that
/// inspection reads the filesystem, and a message builder that did so could not
/// be exercised without one; it also puts the inspection at the boundary that
/// has already decided to refuse.
pub(in crate::nested_cargo::preparation) fn missing_stamp_message(
    preparation: &Preparation,
    stamp: &Path,
    evicted: &[String],
) -> String {
    let mut reason = format!("{} is missing or stale", stamp.display());
    // The same predicate the gate uses, so the message names exactly the
    // directories and artefacts whose absence refused it — including a profile
    // that still exists but has lost `deps`, `.fingerprint` or a recorded unit.
    if !evicted.is_empty() {
        let _ = std::fmt::Write::write_fmt(
            &mut reason,
            format_args!(
                ", and the artefacts it vouched for are gone ({})",
                evicted.join(", ")
            ),
        );
    }
    format!(
        "nested fixture artefacts for family `{}` are not prepared ({reason}).\nUnder nextest \
         every test runs in its own process, so preparing here would make a timed test pay for \
         the cold build it is meant to avoid.\nPrepare them first with `make \
         prepare-nested-fixtures` (or `cargo nextest run -E \
         'test(/nested_fixture_preparation$/)`), then run the filtered selection through `make \
         nextest-run`, which carries the matching environment. Both halves must agree on \
         `RUSTFLAGS` and the feature selection: the fingerprint covers both, so a run staged \
         under a different environment refuses the stamp the preparation just wrote, whatever the \
         timestamp says.",
        preparation.family
    )
}
