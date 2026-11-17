//! The cases that decide when a recorded name has been replaced, not lost.
//!
//! Exercises [`is_vouched_for`] from the parent [`replacement`](super) module,
//! the predicate that reads a hash-carrying name as the same unit under a new
//! hash rather than as a missing one. That module sits between the recorded
//! names and the gate that compares them, so the rule is about Cargo's naming
//! and is stated here at the name level. The sibling eviction cases in the
//! parent `artefacts` module cover the store this protects, where a
//! recompilation by a sibling family must not read as an eviction by the one
//! that recorded it.
//!
//! These exercise the predicate directly rather than through a prepared tree.
//! The distinction it draws is a rule about Cargo's naming, so a name-level
//! case is what specifies it; the store it protects is covered by the parent
//! module's own eviction cases.

use std::collections::BTreeSet;

use super::is_vouched_for;

/// The hash the recorded name carries.
const OLD_HASH: &str = "0123456789abcdef";
/// The hash Cargo would give the same unit after recompiling it.
const NEW_HASH: &str = "fedcba9876543210";

/// The present-name list a check is run against.
fn present(names: &[&str]) -> Vec<String> { names.iter().map(|name| (*name).to_owned()).collect() }

/// The index the predicate consults, built from the same names.
fn index(names: &[String]) -> BTreeSet<&str> { names.iter().map(String::as_str).collect() }

/// The unit the preparation left is still there, so nothing was replaced.
#[test]
fn an_exactly_present_name_is_vouched_for() {
    let recorded = format!("deps/libunit-{OLD_HASH}.rlib");
    let present = present(&[recorded.as_str()]);
    assert!(
        is_vouched_for(&recorded, &index(&present), &[]),
        "a name the preparation recorded and that is still present must pass"
    );
}

/// A recompiled unit answers for the name it replaced.
///
/// This is the shape that failed the workspace suite: both scratch fixtures
/// stamp the shared `<target>/debug`, so each family's stamp also records its
/// sibling's units, and the rebuild experiment runs `cargo clean -p` on a crate
/// the addition family had recorded. Cargo rebuilt those units under new
/// metadata hashes, and the old names were reported missing although the
/// profile still held every unit.
#[test]
fn a_recompiled_unit_answers_for_the_name_it_replaced() {
    let recorded = format!("deps/libunit-{OLD_HASH}.rlib");
    let rebuilt = format!("deps/libunit-{NEW_HASH}.rlib");
    let replacements = present(&[rebuilt.as_str()]);
    assert!(
        is_vouched_for(&recorded, &index(&replacements), &replacements),
        "the same unit under a new metadata hash must not be an eviction"
    );
}

/// Every replacement that must not answer for a recorded name.
///
/// One case per way a candidate can differ from the recorded name, each
/// supplying the recorded name and the candidate the profile would hold for it
/// instead. They share the rejection shape, so stating it once keeps the cases
/// a table of names rather than five copies of the same three lines; the case
/// names are what keep each separately identifiable in a report.
///
/// The names are written out rather than derived from the production rule: a
/// table built from the rule would inherit whatever the rule had started
/// accepting, so a name this table drops could never fail.
///
/// The first case is the one the exact layer exists for. Without it the
/// recompile case would excuse a unit whose `.rlib` was removed while only its
/// `.rmeta` survived, since Cargo recompiles a unit in that state.
///
/// The cases are:
///
/// * `another_output_form` — a surviving `.rmeta` must not witness a removed `.rlib`, which is the
///   hole the exact layer exists to close.
/// * `another_directory` — two units can share a stem, the same crate compiled for a build script
///   and for the library, so the directory is part of what identifies them; without it a
///   build-script unit could witness a missing library unit.
/// * `an_unhashed_name` — an unhashed name is not a unit Cargo recompiles under a new hash, so
///   there is no replacement to recognize; reading one as replaced would excuse a missing artefact
///   on the strength of another entry that merely shares its prefix.
/// * `a_non_hexadecimal_hash` — a stem that itself ends in something hex-like must not be split at
///   the wrong `-`, or `liba-b-0123456789abcdef.rlib` would be credited to stem `liba-b` and excuse
///   a missing `liba-b-*` unit of a different form.
/// * `a_suffix_prefix` — a suffix that merely begins the same way is a different output.
#[rstest::rstest]
#[case::another_output_form(
    "deps/libunit-0123456789abcdef.rlib",
    "deps/libunit-fedcba9876543210.rmeta",
    "a surviving `.rmeta` must not witness a removed `.rlib`"
)]
#[case::another_directory(
    "deps/libunit-0123456789abcdef.rlib",
    "build/unit/fedcba9876543210/out/libunit-fedcba9876543210.rlib",
    "the same stem in another directory is a different unit"
)]
#[case::an_unhashed_name(
    "deps/libunit.rlib",
    "deps/libunit.rlib.rmeta",
    "an unhashed name has no hash to differ in, so no lookalike may replace it"
)]
#[case::a_non_hexadecimal_hash(
    "deps/libunit-0123456789abcdef.rlib",
    "deps/libunit-zzzzzzzzzzzzzzzz.rlib",
    "a 16-character non-hexadecimal tail is not a Cargo metadata hash"
)]
#[case::a_suffix_prefix(
    "deps/libunit-0123456789abcdef.rlib",
    "deps/libunit-fedcba9876543210.rlib.bak",
    "`.rlib.bak` is not the `.rlib` the preparation recorded"
)]
fn a_replacement_in_another_form_does_not_answer_for_a_lost_output(
    #[case] recorded: &str,
    #[case] candidate: &str,
    #[case] why: &str,
) {
    let replacements = present(&[candidate]);
    assert!(
        !is_vouched_for(recorded, &index(&replacements), &replacements),
        "{why}"
    );
}

/// A recorded sibling cannot answer for a name that went.
///
/// A profile can hold two builds of one crate under different hashes — a stale
/// one and a fresh one, or the same crate built for two feature sets. Both are
/// recorded at preparation time. If one `.rlib` is then removed, its recorded
/// sibling is not a replacement for it: it is a separate artefact the stamp
/// already accounts for, and reading it as the missing one's rebuild reports a
/// lost unit as present.
#[test]
fn a_recorded_sibling_does_not_answer_for_a_name_that_went() {
    let recorded = format!("deps/libunit-{OLD_HASH}.rlib");
    let sibling = format!("deps/libunit-{NEW_HASH}.rlib");
    // Both were recorded, so the sibling is not in the replacement set the
    // caller builds from what is present but unrecorded.
    let present = present(&[sibling.as_str()]);
    // The replacement set is what the caller derives by removing every recorded
    // name from what is present, so a recorded sibling leaves nothing behind.
    assert!(
        !is_vouched_for(&recorded, &index(&present), &[]),
        "a name recorded alongside the missing one must not stand in for it"
    );
}

/// A fingerprint record is answered for by the same file under the rebuilt
/// unit's directory.
///
/// Cargo files a unit's records under `<store>/<unit>-<hash>/`, so recompiling
/// the unit renames that directory and leaves the file names inside it alone.
/// Treating the record as an ordinary name would report every rebuilt unit's
/// records as missing, which is the false refusal the compiled-output rule was
/// written to avoid.
#[test]
fn a_rebuilt_units_record_answers_for_the_name_it_replaced() {
    let recorded = format!(".fingerprint/unit-{OLD_HASH}/lib-unit");
    let rebuilt = format!(".fingerprint/unit-{NEW_HASH}/lib-unit");
    let replacements = present(&[rebuilt.as_str()]);
    assert!(
        is_vouched_for(&recorded, &index(&replacements), &replacements),
        "a record filed under the rebuilt unit's directory is the same record"
    );
}

/// A record answered for from another store is another unit's record.
///
/// The store is part of what identifies a record: a profile can hold a
/// profile-level `.fingerprint` and, under Cargo 1.100.0's layout, one per
/// unit. A record in one of them says nothing about a unit in another.
#[test]
fn a_record_in_another_store_does_not_answer_for_the_name() {
    let recorded = format!(".fingerprint/unit-{OLD_HASH}/lib-unit");
    let elsewhere = format!("build/unit/{NEW_HASH}/fingerprint/unit-{NEW_HASH}/lib-unit");
    let replacements = present(&[elsewhere.as_str()]);
    assert!(
        !is_vouched_for(&recorded, &index(&replacements), &replacements),
        "a record in another store is another unit's record"
    );
}

/// A record whose file name differs is another record.
///
/// Cargo writes several records per unit — the fingerprint itself, its JSON
/// sidecar, the dep-info list — and losing one of them is losing a record the
/// build reads. Only the same file name under the rebuilt directory may answer.
#[test]
fn a_record_of_another_kind_does_not_answer_for_the_name() {
    let recorded = format!(".fingerprint/unit-{OLD_HASH}/lib-unit");
    let rebuilt = format!(".fingerprint/unit-{NEW_HASH}/lib-unit.json");
    let replacements = present(&[rebuilt.as_str()]);
    assert!(
        !is_vouched_for(&recorded, &index(&replacements), &replacements),
        "a JSON sidecar is not the fingerprint record that went"
    );
}
