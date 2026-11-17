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

/// A registry crate the fixture's build compiles.
///
/// Registry crates are immutable, so the declaration originally named only path
/// packages — but their units are compiled into the same profile and a removed
/// fingerprint record rebuilds them all the same.
const REGISTRY_PACKAGE: &str = "inventory";

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

#[test]
fn evicting_a_registry_packages_records_invalidates_the_preparation() {
    // A registry crate is immutable, which is why the CLI family's declaration
    // omitted registry packages at first — but "cannot change" is not "cannot be
    // evicted". Removing a registry package's fingerprint records makes Cargo
    // rebuild it, and that rebuild lands inside whichever timed assertion runs
    // next. While the package was undeclared, its records and its present units
    // were filtered out of both sides, so the two empty lists compared equal and
    // the gate admitted the tree.
    let (_directory, mut preparation, _environment) =
        scratch_preparation().expect("the scratch preparation fixture");
    // Both the fixture's own package and the registry one, as the CLI family
    // declares its whole closure. Declaring neither would leave the registry
    // records unowned and the case would pass for the wrong reason.
    preparation.owned_packages = [FIXTURE_PACKAGE, REGISTRY_PACKAGE]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let profile = prepared_profile(&preparation)
        .expect("the preparation describes the profile directory it reuses");

    let fixture_unit = profile
        .join(".fingerprint")
        .join(format!("{FIXTURE_PACKAGE}-0123456789abcdef"));
    let registry_unit = profile
        .join(".fingerprint")
        .join(format!("{REGISTRY_PACKAGE}-0123456789abcdef"));
    for unit in [&fixture_unit, &registry_unit] {
        fs::create_dir_all(unit).expect("create a unit's record directory");
        fs::write(unit.join("record"), b"fingerprint record").expect("write a record");
    }

    prepare_now(&preparation, || Ok(PreparationSummary::default()))
        .expect("prepare over the registry package's records");

    fs::remove_dir_all(&registry_unit).expect("evict the registry package's fingerprint records");
    assert!(
        fixture_unit.is_dir(),
        "the fixture's own records must survive, or this is not the registry case"
    );

    let error = ensure_prepared_with(&preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("a registry package whose records went must fail the gate");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(
        error.to_string().contains(REGISTRY_PACKAGE),
        "the refusal must name the evicted registry package: {error}"
    );
}

#[test]
fn a_declared_package_with_no_recorded_artefact_refuses_the_preparation() {
    // `owned_packages` restricts what a stamp may speak for, so a package named
    // there that the build never produced contributes nothing to the record.
    // Its units would then be absent from both sides — nothing recorded, nothing
    // found — and a later eviction of them would compare equal and pass the
    // gate. A misspelt name and a name left behind by a package rename reach the
    // same state, which is why this is checked against the record rather than
    // against the constant alone.
    let (_directory, mut preparation, _environment) =
        scratch_preparation().expect("the scratch preparation fixture");
    preparation.owned_packages = [FIXTURE_PACKAGE, "never-built-package"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    // The fixture's own package must contribute a record, or the refusal would
    // be about it too and the case would not show that one undeclared name is
    // enough to fail the preparation.
    let fixture_unit = prepared_profile(&preparation)
        .expect("the preparation describes the profile directory it reuses")
        .join(".fingerprint")
        .join(format!("{FIXTURE_PACKAGE}-0123456789abcdef"));
    fs::create_dir_all(&fixture_unit).expect("create the fixture's record directory");
    fs::write(fixture_unit.join("record"), b"fingerprint record").expect("write a record");

    let error = prepare_now(&preparation, || Ok(PreparationSummary::default()))
        .expect_err("a declared package that contributed nothing must be refused");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    let message = error.to_string();
    assert!(
        message.contains("never-built-package"),
        "the refusal must name the package that contributed nothing: {message}"
    );
    assert!(
        !message.contains(FIXTURE_PACKAGE),
        "the fixture's own package did contribute, so it must not be named: {message}"
    );
    assert!(
        !stamp_path(&preparation).is_file(),
        "a refused preparation must not leave a stamp vouching for the tree"
    );
}

/// The name a package's unit stem comes from, which need not be the package.
///
/// A package `adapter` that declares `[lib] name = "engine"` files its
/// fingerprint records under `adapter` and writes its output as `libengine`, so
/// the two names are both needed to cover the one unit.
const ADAPTER_PACKAGE: &str = "adapter";
const ENGINE_TARGET: &str = "engine";

/// Removing a target-named output invalidates the stamp that recorded it.
///
/// The regression the predicate's own tests cannot reach: they ask whether a
/// name is owned, while this asks whether a *removal* is seen. It is the case
/// that fails if a caller declares a package and assumes that covers the unit,
/// because `name_is_owned` matches the names it is given and derives neither
/// from the other. The output goes under the target name while the records stay
/// under the package name, so the records alone keep the profile looking
/// populated and owned — only the output's own recorded name can report it
/// missing.
///
/// Everything except that one file is left in place: the fingerprint store
/// stays intact and usable, so a refusal cannot come from the structural layer
/// that a wholesale `cargo clean` would trip.
#[test]
fn evicting_a_target_named_output_invalidates_the_preparation() {
    let (_directory, mut preparation, _environment) =
        scratch_preparation().expect("the scratch preparation fixture");
    // Both names, because they cover different halves of one unit. Declaring
    // `engine` alone would leave the records unowned; declaring `adapter` alone
    // — the mistake this case exists for — leaves the output unowned.
    preparation.owned_packages = [ADAPTER_PACKAGE, ENGINE_TARGET]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let profile = prepared_profile(&preparation)
        .expect("the preparation describes the profile directory it reuses");

    // The output is named for the target, the records for the package, exactly
    // as Cargo writes them when the two disagree. The `.d` record is what makes
    // the directory a *unit* directory at all: discovery classifies a directory
    // by the dep-info it holds, so an output written alone would never be
    // recorded and the removal would be invisible for a reason that has nothing
    // to do with ownership.
    //
    // The record is named `<stem>.d` beside `lib<stem>.rlib`, not
    // `lib<stem>.d`: rustc spells the dep-info with the unit's own stem, which
    // carries no `lib`, and only then does `is_output_of` pair the two through
    // its *library* branch. The unprefixed form is also what the shared
    // `write_unit` helper writes. A `lib`-prefixed record instead makes the
    // stem `libengine-<hash>`, which the output matches by its *binary* branch,
    // a naming rustc does not produce and the helper does not model.
    let deps = profile.join("deps");
    fs::create_dir_all(&deps).expect("create the unit directory");
    let unit = format!("{ENGINE_TARGET}-0123456789abcdef");
    let output = deps.join(format!("lib{unit}.rlib"));
    fs::write(&output, b"unit").expect("write the target-named output");
    fs::write(deps.join(format!("{unit}.d")), b"dep-info").expect("write the dep-info record");
    let records = profile
        .join(".fingerprint")
        .join(format!("{ADAPTER_PACKAGE}-0123456789abcdef"));
    fs::create_dir_all(&records).expect("create the record directory");
    fs::write(records.join("lib-engine"), b"fingerprint record").expect("write a record");

    prepare_now(&preparation, || Ok(PreparationSummary::default()))
        .expect("prepare the family over its target-named unit");
    assert!(
        stamp_path(&preparation).is_file(),
        "the stamp must be written"
    );

    fs::remove_file(&output).expect("evict the target-named output");
    assert!(
        records.is_dir(),
        "the records must survive, or the structural layer could refuse instead"
    );

    let error = ensure_prepared_with(&preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("a target-named output that went must fail the gate");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    let message = error.to_string();
    assert!(
        message.contains(ENGINE_TARGET),
        "the message must name the evicted output, not just the profile: {message}"
    );
}
