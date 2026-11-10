//! A stamp that is present but cannot be read.
//!
//! This is the state neither "matching" nor "missing" describes, and the two
//! runners must reach different conclusions from it: under `nextest` it is a
//! refusal, because the tree may well be warm and the test process cannot
//! rebuild it; under plain `cargo test` it is a rebuild, because refusing
//! would leave a corrupt file with no in-process remedy. `prepare_now` has no
//! runner to consult at all, so it always replaces.

use std::fs;

use super::{super::stamp::stamp_path, scratch_preparation};
use crate::nested_cargo::{Preparation, PreparationSummary, ensure_prepared_with, prepare_now};

/// Write a stamp that exists but cannot be read as text.
///
/// Invalid UTF-8 makes `read_to_string` fail with `InvalidData`, which is the
/// same "present but unreadable" state a permission bit would produce, without
/// depending on the permission model — a suite running as root can read a file
/// whose mode is zero. The file is left in place rather than made a directory
/// so a later preparation can replace it, which is what the `cargo test` case
/// below needs to reach its successful end.
fn write_unreadable_stamp(preparation: &Preparation) {
    fs::write(stamp_path(preparation), [0xff, 0xfe]).expect("unreadable stamp");
}

#[test]
fn nextest_reports_an_unreadable_stamp_rather_than_calling_it_missing() {
    // The two runners need opposite answers from an unreadable stamp, so the
    // branch that fails fast exists only at this boundary: the query in
    // `stamp` returns the error and decides nothing. Without this test the
    // nextest branch is unexercised, and it would be possible to report
    // "missing or stale" for a tree that is present but cannot be vouched
    // for — which sends the reader looking for an evicted directory.
    let (_directory, preparation, _environment) = scratch_preparation();
    write_unreadable_stamp(&preparation);

    let error = ensure_prepared_with(&preparation, true, || {
        panic!("an unreadable stamp under nextest must not prepare")
    })
    .expect_err("an unreadable stamp must refuse the gate");
    let message = error.to_string();
    assert!(message.contains("exists but cannot be read"), "{message}");
    assert!(
        message.contains("make prepare-nested-fixtures"),
        "the message must still name the remedy: {message}"
    );
    assert!(
        !message.contains("missing or stale"),
        "an unreadable stamp must not be reported as a missing one: {message}"
    );
}

#[test]
fn cargo_test_prepares_again_behind_an_unreadable_stamp() {
    // The complement of the case above: under `cargo test` the same fact only
    // costs a rebuild, so the boundary must choose to prepare rather than
    // fail. Running the closure is what proves the branch was taken — had the
    // boundary failed fast, it would not have run.
    let (_directory, preparation, _environment) = scratch_preparation();
    write_unreadable_stamp(&preparation);
    let mut runs = 0_u32;

    ensure_prepared_with(&preparation, false, || {
        runs += 1;
        Ok(PreparationSummary { compiled_units: 1 })
    })
    .expect("cargo test must recover from an unreadable stamp by preparing");

    assert_eq!(
        runs, 1,
        "under cargo test an unreadable stamp must lead to preparation, not a refusal"
    );
    // The replacement stamp is readable, so the next gate reuses it rather
    // than paying for the build again. An unreadable stamp that survived the
    // preparation would cost a rebuild on every single run.
    let mut runs_after = runs;
    ensure_prepared_with(&preparation, false, || {
        runs_after += 1;
        Ok(PreparationSummary::default())
    })
    .expect("the stamp written by the recovery must be reusable");
    assert_eq!(runs_after, runs, "the recovered stamp must be honoured");
}

#[test]
fn prepare_now_replaces_an_unreadable_stamp_and_leaves_a_reusable_one() {
    // `prepare_now` is the unconditional entry point the `make
    // prepare-nested-fixtures` target reaches, and it has no runner to consult:
    // it is always asked to prepare. An unreadable stamp must therefore not
    // stop it, and the stamp it writes must be *usable* afterwards — a
    // replacement that were itself unreadable would make every later gate
    // re-prepare forever, which is the failure mode the recovery exists to
    // prevent but one layer down.
    let (_directory, preparation, _environment) = scratch_preparation();
    write_unreadable_stamp(&preparation);
    let mut builds = 0_u32;

    prepare_now(&preparation, || {
        builds += 1;
        Ok(PreparationSummary::default())
    })
    .expect("prepare_now must replace an unreadable stamp rather than refuse");

    assert_eq!(builds, 1, "an unreadable stamp must not count as prepared");

    // The stamp left behind describes the tree, so the gate accepts it without
    // running the closure again.
    let mut rebuilds = builds;
    ensure_prepared_with(&preparation, false, || {
        rebuilds += 1;
        Ok(PreparationSummary::default())
    })
    .expect("the stamp prepare_now wrote must be reusable");
    assert_eq!(
        rebuilds, builds,
        "prepare_now must leave a stamp the gate can read"
    );
}
