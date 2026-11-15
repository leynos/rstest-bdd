//! Ownership: which evictions a family that shares a profile must refuse.
//!
//! A family declares the packages its preparation built, so a sibling's units
//! stay outside its stamp. These cases cover the boundary of that declaration:
//! the protection must exclude an unrelated sibling, and must not exclude the
//! workspace crates the family's own build produced.

use std::{fs, io};

use super::{super::stamp::stamp_path, prepared_profile, scratch_preparation};
use crate::nested_cargo::{PreparationSummary, ensure_prepared_with, prepare_now};

/// The CLI fixture package, and one workspace crate its build also produces.
///
/// The dependency case below turns on the pair: declaring the fixture alone
/// leaves the dependency filtered out of both name lists.
const FIXTURE_PACKAGE: &str = "rstest-bdd-cli-fixture";
const DEPENDENCY_PACKAGE: &str = "rstest-bdd";

#[test]
fn evicting_a_dependency_owned_through_the_fixture_invalidates_the_preparation() {
    // The CLI family shares the workspace `target/debug`, so it cannot claim the
    // whole profile: a name belonging to an unrelated package stays filtered out
    // of what its stamp records. That protection must stop at the fixture's own
    // path dependencies, whose units its preparation *did* build and which Cargo
    // rebuilds inside the timed test if their fingerprints go. Recording nothing
    // for them would let this profile — its fixture units and every sibling unit
    // still intact — pass a gate that an evicted `rstest-bdd` should fail.
    let (_directory, mut preparation, _environment) =
        scratch_preparation().expect("the scratch preparation fixture");
    // `scratch_preparation` leaves `owned_packages` empty, which claims the
    // whole profile. Without narrowing it, every name is owned and this case
    // would pass for a reason that has nothing to do with ownership — the
    // profile is shared, and declaring only the fixture is what the CLI family
    // used to do.
    preparation.owned_packages = [FIXTURE_PACKAGE, DEPENDENCY_PACKAGE]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let profile = prepared_profile(&preparation)
        .expect("the preparation describes the profile directory it reuses");
    let store = profile.join(".fingerprint");

    // The fixture's own units, written beside the dependency's records: the
    // profile must still look populated and owned from above, so the refusal
    // has to come from the dependency record check.
    let fixture_unit = store.join(format!("{FIXTURE_PACKAGE}-0123456789abcdef"));
    let dependency_unit = store.join("rstest-bdd-0123456789abcdef");
    for unit in [&fixture_unit, &dependency_unit] {
        fs::create_dir_all(unit).expect("create a unit's record directory");
        fs::write(unit.join("lib-unit"), b"fingerprint record").expect("write a record");
    }

    // The stamp has to exist before the eviction, and it can only be written by
    // a preparation that ran over this tree. Under nextest the gate itself is
    // fail-fast, so the family is prepared through the unconditional entry
    // point — exactly as `make prepare-nested-fixtures` reaches it.
    prepare_now(&preparation, || Ok(PreparationSummary::default()))
        .expect("prepare the narrowed family over its fixture units");
    assert!(
        stamp_path(&preparation).is_file(),
        "the stamp must be written"
    );

    fs::remove_dir_all(&dependency_unit).expect("evict the dependency's fingerprint records");
    assert!(
        fixture_unit.is_dir(),
        "the fixture's own records must survive, or this is not the ownership case"
    );

    let error = ensure_prepared_with(&preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("a dependency whose records went must fail the gate");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    let message = error.to_string();
    assert!(
        message.contains("rstest-bdd-0123456789abcdef"),
        "the message must name the evicted dependency, not just the profile: {message}"
    );
}
