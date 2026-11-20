//! Entry points that stage a family's working directory before identifying it.
//!
//! Both gates identify the toolchain before the preparation callback runs,
//! because the identity is part of the fingerprint a stamp is compared with.
//! That order is wrong for a family whose callback *creates* the directory its
//! nested builds run in. On the cold run the directory is absent, so the probe
//! starts in its nearest existing ancestor: a relative `RUSTC` beneath the
//! directory names a file that is not there yet, and a `rust-toolchain.toml`
//! the callback is about to copy in is invisible to the shim that would select
//! from it. The stamp then records a compiler the nested build does not run,
//! and the first warm gate, which probes from the directory itself, refuses the
//! stamp that preparation just wrote.
//!
//! Staging is the remedy: the family copies the tree its builds run in *first*,
//! as a step of its own, and only then is it identified. The step is the
//! family's because only it knows what the tree holds; these functions fix the
//! order so no call site can identify before staging.

use std::io;

use super::{PreparationOutcome, ensure_prepared_building, prepare_now};
use crate::nested_cargo::preparation::{Preparation, PreparationSummary};

/// [`prepare_now`], with the working directory staged before it is identified.
///
/// `stage` must leave [`Preparation::working_directory`] holding everything
/// that selects a toolchain there: the directory itself, any toolchain file,
/// and any compiler a relative `RUSTC` names. It runs unconditionally and
/// before the stamp is read, so it must be idempotent and must not build.
///
/// # Errors
///
/// Returns `stage`'s error without identifying or preparing anything, and
/// otherwise as [`prepare_now`].
pub fn prepare_now_staged<S, F>(preparation: &Preparation, stage: S, prepare: F) -> io::Result<()>
where
    S: FnOnce() -> io::Result<()>,
    F: FnOnce() -> io::Result<PreparationSummary>,
{
    stage()?;
    prepare_now(preparation, prepare)
}

/// [`ensure_prepared_building`], staged first wherever it is allowed to build.
///
/// Under `nextest` the gate never prepares, so it does not stage either: a
/// timed test must not write the tree it is about to judge, and an unstaged
/// family is refused there exactly as an unprepared one is. Under plain
/// `cargo test` the gate may build, and `stage` runs before the toolchain is
/// identified for the reason this module gives.
///
/// # Errors
///
/// Returns `stage`'s error when it runs and fails, and otherwise as
/// [`ensure_prepared_building`].
pub fn ensure_prepared_building_staged<S, F>(
    preparation: &Preparation,
    under_nextest: bool,
    stage: S,
    prepare: F,
) -> io::Result<PreparationOutcome>
where
    S: FnOnce() -> io::Result<()>,
    F: FnOnce() -> io::Result<PreparationSummary>,
{
    if !under_nextest {
        stage()?;
    }
    ensure_prepared_building(preparation, under_nextest, prepare)
}
