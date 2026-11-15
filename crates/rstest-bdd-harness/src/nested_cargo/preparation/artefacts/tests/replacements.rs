//! Gate-level replacement cases: what the caller's replacement set must exclude.
//!
//! [`replacement::is_vouched_for`] decides whether a recorded name that is no
//! longer present was superseded by a rebuild. The set it is given is built by
//! the *caller*, `artefacts::eviction_of`, which removes every name the stamp
//! recorded from what is present. `replacement`'s own tests pin the predicate;
//! the cases here pin the call site, because that is where the exclusion lives
//! and a predicate test that passes `&[]` by hand cannot reach it.
//!
//! Both cases use hashed names on purpose. An unhashed name such as
//! `deps/libunit.rlib` carries no Cargo unit component, so `UnitPath::parse`
//! refuses it and `is_vouched_for` returns before it looks at a replacement. A
//! case written with one therefore passes whether or not the call site excludes
//! recorded names, which is exactly the coverage gap these two close: one shows
//! a recorded sibling is *not* read as a rebuild, the other that a genuine
//! unrecorded rebuild still is.
//!
//! [`replacement::is_vouched_for`]: super::super::replacement::is_vouched_for

use std::fs;

use super::{preparation_with, profile_of};
use crate::nested_cargo::{
    Preparation,
    preparation::{
        artefacts::{Eviction, evicted, record},
        stamp::Stamp,
    },
};

/// A profile holding the metadata-only unit that keeps it structurally usable.
///
/// Both cases remove an `.rlib`; without a second unit the profile would lose
/// its last compiled output and the structural layer would refuse for a reason
/// that has nothing to do with replacements, hiding what is under test.
fn profile_with_metadata_unit() -> (tempfile::TempDir, Preparation) {
    preparation_with(&[("checked", "rmeta")])
}

/// Write one build of the unit under `hash`, as rustc leaves it.
///
/// A `.d` record beside an `.rlib`, sharing one hashed stem: that pairing is
/// what makes the directory listing discover the output at all.
fn write_build(deps: &std::path::Path, hash: &str) {
    fs::write(deps.join(format!("unit-{hash}.d")), b"dep-info").expect("dep-info record");
    fs::write(deps.join(format!("libunit-{hash}.rlib")), b"unit").expect("compiled output");
}

/// Two recorded builds of one crate, one removed: the survivor must not answer.
///
/// A profile can hold one crate compiled twice under different metadata hashes —
/// a stale build and a fresh one. Recording both output names is what the exact
/// layer does, so when one `.rlib` goes, its recorded sibling is *not* a
/// replacement: it is a separate artefact the stamp already accounts for.
/// Reading it as the missing one's rebuild reports a lost unit as present at
/// exactly the moment Cargo recompiles.
#[test]
fn a_recorded_sibling_does_not_answer_for_a_removed_hashed_build() {
    const REMOVED_HASH: &str = "0123456789abcdef";
    const SIBLING_HASH: &str = "fedcba9876543210";

    let (directory, preparation) = profile_with_metadata_unit();
    let deps = profile_of(&preparation).join("deps");
    write_build(&deps, REMOVED_HASH);
    write_build(&deps, SIBLING_HASH);

    let recorded = record(&preparation).expect("record");
    let removed = format!("deps/libunit-{REMOVED_HASH}.rlib");
    let sibling = format!("deps/libunit-{SIBLING_HASH}.rlib");
    for name in [&removed, &sibling] {
        assert!(
            recorded.iter().flatten().any(|recorded| recorded == name),
            "both builds must be recorded, or the sibling is not the case under test: {recorded:?}"
        );
    }

    fs::remove_file(deps.join(format!("libunit-{REMOVED_HASH}.rlib"))).expect("remove one build");
    assert!(
        deps.join(format!("libunit-{SIBLING_HASH}.rlib")).is_file(),
        "the recorded sibling must survive, so it can be the thing that must not answer"
    );

    let stamp = Stamp { recorded };
    let evictions = evicted(&preparation, Some(&stamp)).expect("check");
    let [Eviction { missing, .. }] = evictions.as_slice() else {
        panic!("exactly one profile must be evicted, got {evictions:?}");
    };
    assert_eq!(
        missing,
        &vec![removed],
        "a hashed build recorded alongside the missing one must not stand in for it"
    );
    assert!(directory.path().exists(), "the fixture must stay alive");
}

/// The control: a rebuild the stamp never recorded *is* accepted.
///
/// Without this, the case above would pass for a check that had simply stopped
/// treating a new generation as a replacement — every rebuilt unit would then be
/// reported missing, and the rebuild experiment the scratch families exist for
/// could never run. Together the pair shows the call site excludes recorded
/// names without excluding the rebuilds it is supposed to accept.
#[test]
fn an_unrecorded_rebuild_answers_for_the_name_it_replaced() {
    const REMOVED_HASH: &str = "0123456789abcdef";
    const SIBLING_HASH: &str = "fedcba9876543210";
    const REBUILT_HASH: &str = "0011223344556677";

    let (directory, preparation) = profile_with_metadata_unit();
    let deps = profile_of(&preparation).join("deps");
    write_build(&deps, REMOVED_HASH);
    write_build(&deps, SIBLING_HASH);
    let recorded = record(&preparation).expect("record");

    // The recompile Cargo performs: the old generation goes and a new one the
    // stamp has never seen takes its place.
    fs::remove_file(deps.join(format!("libunit-{REMOVED_HASH}.rlib")))
        .expect("remove the superseded build");
    fs::remove_file(deps.join(format!("unit-{REMOVED_HASH}.d")))
        .expect("remove the superseded record");
    write_build(&deps, REBUILT_HASH);
    assert!(
        !recorded
            .iter()
            .flatten()
            .any(|name| name.contains(REBUILT_HASH)),
        "the rebuild must be a name the stamp never recorded, or this is not the control: \
         {recorded:?}"
    );

    let stamp = Stamp { recorded };
    let evictions = evicted(&preparation, Some(&stamp)).expect("check");
    assert!(
        evictions.is_empty(),
        "a unit rebuilt under a new hash must satisfy the gate, got {evictions:?}"
    );
    assert!(directory.path().exists(), "the fixture must stay alive");
}
