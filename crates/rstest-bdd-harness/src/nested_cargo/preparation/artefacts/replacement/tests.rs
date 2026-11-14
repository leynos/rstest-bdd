//! The cases that decide when a recorded name has been replaced, not lost.
//!
//! These exercise the predicate directly rather than through a prepared tree.
//! The distinction it draws is a rule about Cargo's naming, so a name-level
//! case is what specifies it; the store it protects is covered by the parent
//! module's own eviction cases.

use super::is_vouched_for;

/// The hash the recorded name carries.
const OLD_HASH: &str = "0123456789abcdef";
/// The hash Cargo would give the same unit after recompiling it.
const NEW_HASH: &str = "fedcba9876543210";

/// The present-name list a check is run against.
fn present(names: &[&str]) -> Vec<String> { names.iter().map(|name| (*name).to_owned()).collect() }

/// The unit the preparation left is still there, so nothing was replaced.
#[test]
fn an_exactly_present_name_is_vouched_for() {
    let recorded = format!("deps/libunit-{OLD_HASH}.rlib");
    assert!(
        is_vouched_for(&recorded, &present(&[recorded.as_str()])),
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
    assert!(
        is_vouched_for(&recorded, &present(&[rebuilt.as_str()])),
        "the same unit under a new metadata hash must not be an eviction"
    );
}

/// A replacement carrying another output form does not answer for the one that
/// went.
///
/// Without this the previous case would excuse a unit whose `.rlib` was removed
/// while only its `.rmeta` survived — reopening precisely the hole the exact
/// layer exists to close, since Cargo recompiles a unit in that state.
#[test]
fn a_replacement_in_another_form_does_not_answer_for_a_lost_output() {
    let recorded = format!("deps/libunit-{OLD_HASH}.rlib");
    let surviving = format!("deps/libunit-{NEW_HASH}.rmeta");
    assert!(
        !is_vouched_for(&recorded, &present(&[surviving.as_str()])),
        "a surviving `.rmeta` must not witness a removed `.rlib`"
    );
}

/// A replacement in another directory is another unit.
///
/// Two units can share a stem — the same crate compiled for a build script and
/// for the library — so the directory is part of what identifies them. Without
/// it, a build-script unit could witness a missing library unit.
#[test]
fn a_replacement_in_another_directory_does_not_answer_for_the_name() {
    let recorded = format!("deps/libunit-{OLD_HASH}.rlib");
    let elsewhere = format!("build/unit/{NEW_HASH}/out/libunit-{NEW_HASH}.rlib");
    assert!(
        !is_vouched_for(&recorded, &present(&[elsewhere.as_str()])),
        "the same stem in another directory is a different unit"
    );
}

/// A name that carries no metadata hash is never answered for by a lookalike.
///
/// An unhashed name is not a unit Cargo recompiles under a new hash, so there
/// is no replacement to recognise. Reading one as replaced would excuse a
/// genuinely missing artefact on the strength of some other entry that happens
/// to share its prefix.
#[test]
fn an_unhashed_name_is_never_answered_for_by_another() {
    assert!(
        !is_vouched_for("deps/libunit.rlib", &present(&["deps/libunit.rlib.rmeta"])),
        "an unhashed name has no hash to differ in, so no lookalike may replace it"
    );
}

/// The hash has to be Cargo's hexadecimal form, not merely short text.
///
/// A stem that itself ends in something hex-like must not be split at the wrong
/// `-`, or `liba-b-0123456789abcdef.rlib` would be credited to stem `liba-b`
/// and excuse a missing `liba-b-*` unit of a different form.
#[test]
fn a_non_hexadecimal_hash_is_not_a_cargo_unit() {
    let recorded = format!("deps/libunit-{OLD_HASH}.rlib");
    let lookalike = "deps/libunit-zzzzzzzzzzzzzzzz.rlib";
    assert!(
        !is_vouched_for(&recorded, &present(&[lookalike])),
        "a 16-character non-hexadecimal tail is not a Cargo metadata hash"
    );
}

/// A suffix that merely begins the same way is a different output.
#[test]
fn a_suffix_prefix_is_not_the_same_output_form() {
    let recorded = format!("deps/libunit-{OLD_HASH}.rlib");
    let lookalike = format!("deps/libunit-{NEW_HASH}.rlib.bak");
    assert!(
        !is_vouched_for(&recorded, &present(&[lookalike.as_str()])),
        "`.rlib.bak` is not the `.rlib` the preparation recorded"
    );
}
