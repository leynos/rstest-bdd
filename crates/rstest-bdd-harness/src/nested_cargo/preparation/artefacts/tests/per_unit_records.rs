//! The fingerprint records a per-unit store holds directly.
//!
//! Cargo 1.100.0 moved every unit into `<profile>/build/<pkg>/<hash>/`, with a
//! `fingerprint` directory beside its `out`. That store belongs to one unit, so
//! its records are files directly inside it. The classic `.fingerprint` store
//! holds a *directory* per unit instead, and a reader written for that shape
//! found no directories in a per-unit store, recorded nothing, and let the
//! removal of a record pass. These cases use the shape Cargo writes.

use std::fs;

use super::{preparation_with_new_layout, profile_of, write_per_unit_store, write_units};
use crate::nested_cargo::{
    PreparationSummary,
    ensure_prepared_building,
    preparation::artefacts::record,
    prepare_now,
};

/// The per-unit store the new-layout fixture writes, relative to the profile.
const STORE: &str = "build/unit/0123456789abcdef/fingerprint";

#[test]
fn a_per_unit_stores_own_files_are_recorded() -> std::io::Result<()> {
    let (_directory, preparation) = preparation_with_new_layout(&[("unit", "rlib")])?;
    let recorded = record(&preparation)?;
    let names = recorded.first().expect("the fixture describes one profile");
    for record_file in ["lib-unit", "lib-unit.json", "dep-lib-unit"] {
        let expected = format!("{STORE}/{record_file}");
        assert!(
            names.contains(&expected),
            "{expected} is a record Cargo reads and must be in the stamp: {names:?}"
        );
    }
    Ok(())
}

#[test]
fn removing_one_record_from_a_per_unit_store_fails_the_gate() -> std::io::Result<()> {
    // The unit's compiled output survives, and so does a second unit with a
    // store of its own, so the profile stays structurally usable throughout.
    // Only the recorded names can report that one record has gone.
    let (_directory, preparation) = preparation_with_new_layout(&[("unit", "rlib")])?;
    let profile = profile_of(&preparation)?;
    let other = profile.join("build/other/fedcba9876543210");
    write_units(&other.join("out"), &[("other", "rlib")])?;
    write_per_unit_store(&other.join("fingerprint"), "other")?;

    prepare_now(&preparation, || Ok(PreparationSummary::default()))?;
    ensure_prepared_building(&preparation, true, || {
        panic!("a just-prepared family must be reused")
    })?;

    fs::remove_file(profile.join(STORE).join("lib-unit"))?;
    assert!(
        profile
            .join("build/unit/0123456789abcdef/out/libunit.rlib")
            .is_file(),
        "the unit's output must survive, or this is not the record case"
    );

    let error = ensure_prepared_building(&preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("a removed fingerprint record must fail the gate");
    assert!(
        error.to_string().contains("lib-unit"),
        "the refusal must name the record that went: {error}"
    );
    Ok(())
}
