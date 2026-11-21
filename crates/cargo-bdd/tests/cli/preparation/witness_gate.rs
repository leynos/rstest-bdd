//! The witness the CLI family asserts on, and its remedy.
//!
//! Preparing a family leaves a stamp the moment its commands succeed, which is
//! before anything can read what they produced. A preparation that exits zero
//! while leaving the fixture's own test binaries out would therefore leave a
//! stamp behind, and a stamp-only gate would admit every timed assertion after
//! it against a tree with nothing to inspect. The predicate here is what both
//! the timed gate and the named preparation test apply for that reason.
//!
//! Split from `preparation.rs` so that file keeps its share of the repository's
//! line budget. The two functions are one concern — decide whether the tree is
//! usable, and withdraw the stamp that said it was — and neither is reachable
//! from outside this test binary.

use eyre::Result;

use super::{nested_cargo, witness::fixture_binaries_present};

/// Refuse a preparation whose profile lacks the fixture binaries, and withdraw
/// the stamp that admitted it.
///
/// Both callers reach a preparation through a gate that returns early from a
/// *matching* stamp: [`nested_cargo::ensure_prepared_building`] on the lazy path and
/// [`nested_cargo::prepare_now`] in the named test. The stamp is written by the
/// preparation, before this witness can be read, so a preparation that exits
/// zero while leaving the fixture's own units out leaves a stamp vouching for a
/// tree no timed assertion can use. Reporting that alone would make every later
/// attempt read the same stamp and fail the same way, with the preparation never
/// running again — the failure would be permanent rather than transient.
///
/// Removing the stamp first is what makes the next attempt a build. The
/// removal's outcome is reported alongside the refusal rather than instead of
/// it: the missing binaries are the failure, and a stamp that could not be
/// withdrawn is a second fact the reader needs, not a substitute for the first.
pub(super) fn witness_or_withdraw(preparation: &nested_cargo::Preparation) -> Result<()> {
    let error = match require_fixture_binaries(preparation) {
        Ok(()) => return Ok(()),
        Err(error) => error,
    };
    let recovery = match nested_cargo::invalidate(preparation) {
        Ok(()) => "the stamp was withdrawn, so the next run prepares again".to_owned(),
        Err(removal) => format!(
            "the stamp could not be withdrawn, so the next run will repeat this failure: {removal}"
        ),
    };
    Err(eyre::eyre!("{error:#}\n{recovery}"))
}

/// Fail unless `preparation`'s selected profile holds the fixture's own test
/// binaries.
///
/// Shared by the timed gate and the named preparation test so the two cannot
/// disagree about what "prepared" means: the predicate the preparation proves
/// is the one the timed assertions require.
///
/// Only the *selected* profile is read, which is the last one the family
/// names. Without a target selection that is `<target>/debug`, the only
/// profile there is. A build selected onto a triple writes the fixture's test
/// binaries to `<target>/<triple>/debug` and only its build scripts and proc
/// macros to `<target>/debug`, and [`nested_cargo::Preparation::new`] names
/// the host profile first and the triple's second.
///
/// Accepting either profile was the earlier rule, and it admitted a tree the
/// timed command would rebuild: binaries a previous host build left in
/// `<target>/debug` satisfied the witness after the triple's own had been
/// removed, although a selected build never reads the host copies. The
/// selection decides which profile Cargo reads, so it decides which profile
/// the witness reads.
///
/// # Errors
///
/// Returns the listing error when the selected profile cannot be read, and an
/// error naming that profile when it is readable and missing a fixture binary.
pub(super) fn require_fixture_binaries(preparation: &nested_cargo::Preparation) -> Result<()> {
    let Some(profile) = preparation.artefacts.last() else {
        return Err(eyre::eyre!(
            "the CLI family must describe the profile its units land in"
        ));
    };
    let present = fixture_binaries_present(profile).map_err(|error| {
        eyre::Report::new(error).wrap_err(format!(
            "listing the prepared profile {}",
            profile.display()
        ))
    })?;
    if present {
        return Ok(());
    }
    Err(eyre::eyre!(
        "preparation must leave the fixture's own test binaries in {}; the timed CLI assertions \
         read them, and a preparation that compiled nothing leaves the suite with nothing to \
         inspect",
        profile.display()
    ))
}
