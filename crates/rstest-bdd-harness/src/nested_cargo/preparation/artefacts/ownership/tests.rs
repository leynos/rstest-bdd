//! Unit tests for the ownership predicate.
//!
//! Exercises [`name_is_owned`], [`strip_unit_suffix`] and [`is_metadata_hash`]
//! from the parent [`ownership`](super) module, which decides whether a name
//! found in a profile belongs to a package a family declared. It is a rule
//! about Cargo's naming — a package hyphenated in its record directory and
//! underscored in its output, a target named differently from its package —
//! so the cases state it at the name level. The eviction cases that rely on
//! it, and the declaration check that holds `owned_packages` to what was
//! actually built, are in the sibling `tests` module beside this one.

use rstest::rstest;

use super::{is_metadata_hash, name_is_owned, strip_unit_suffix};

/// The package the addition family declares as its own.
///
/// The sibling's name is spelled inside the cases that need it rather than
/// declared here: it appears only as literal unit names, which the constant
/// could not build anyway, and a second declaration would be a copy to keep in
/// step for no gain.
const ADDITION: &str = "rstest-bdd-feature-addition-fixture";

/// The owned package set a case states its claim against.
///
/// A `&[&str]` of names rather than a `Vec<String>`, because every case states
/// its declaration literally and the predicate wants the owned half only as a
/// slice to search.
fn owned(names: &[&str]) -> Vec<String> { names.iter().copied().map(str::to_owned).collect() }

/// A package's directory form and its crate-name form are both its units.
///
/// Cargo hyphenates the package for `.fingerprint/<package>-<hash>/` and
/// underscores it for `deps/lib<stem>-<hash>.rlib`, so a rule that knew only
/// one spelling would leave half of a family's own units unrecorded — and the
/// unchecked half is where a `cargo clean -p` would go unnoticed. Each spelling
/// is its own case so a failure names the shape that stopped being recognized.
#[rstest]
#[case::fingerprint_directory(
    ".fingerprint/rstest-bdd-feature-addition-fixture-0123456789abcdef/invoked.timestamp"
)]
#[case::underscored_rlib("deps/librstest_bdd_feature_addition_fixture-0123456789abcdef.rlib")]
#[case::underscored_binary("deps/rstest_bdd_feature_addition_fixture-0123456789abcdef")]
#[case::build_script_directory(
    "build/rstest-bdd-feature-addition-fixture-0123456789abcdef/out/build-script-build"
)]
fn both_spellings_of_a_package_are_recognized(#[case] name: &str) {
    assert!(
        name_is_owned(name, &owned(&[ADDITION])),
        "`{name}` belongs to the package"
    );
}

/// A sibling fixture's units are not this family's to vouch for.
///
/// This is the regression: both scratch families build into the shared
/// `target/debug`, and the rebuild experiment runs `cargo clean -p` on its own
/// fixture before every baseline. A stamp that recorded the sibling's units was
/// refused for ever afterwards. The three cases are the three shapes
/// `cargo clean -p` removes: a record directory, a compiled output and a bare
/// unit name.
#[rstest]
#[case::record_directory(
    ".fingerprint/rstest-bdd-rebuild-invalidation-fixture-0123456789abcdef/\
     lib-rstest_bdd_rebuild_invalidation_fixture"
)]
#[case::compiled_output("deps/librstest_bdd_rebuild_invalidation_fixture-0123456789abcdef.rlib")]
#[case::target_named_unit("deps/invalidation-0123456789abcdef")]
fn a_sibling_packages_units_are_not_owned(#[case] name: &str) {
    assert!(
        !name_is_owned(name, &owned(&[ADDITION])),
        "`{name}` belongs to the sibling, not to this family"
    );
}

/// A name that merely shares a prefix or a substring is not the package.
///
/// `rstest-bdd` and `rstest-bdd-feature-addition-fixture` both build into the
/// shared profile, and the shorter name is a prefix of the longer one's crate
/// stem. A substring test would claim every one of the longer package's units
/// for the shorter package — and, worse, would claim a sibling's units for it.
/// The macro crate is the third case because its stem extends `rstest_bdd` by
/// an underscore rather than a hyphen.
#[rstest]
#[case::shorter_rlib("deps/librstest_bdd-0123456789abcdef.rlib")]
#[case::shorter_dep_info("deps/rstest_bdd-0123456789abcdef.d")]
#[case::macro_crate("deps/librstest_bdd_macros-0123456789abcdef.rlib")]
fn a_prefix_of_a_package_name_is_another_package(#[case] name: &str) {
    assert!(
        !name_is_owned(name, &owned(&[ADDITION])),
        "`{name}` is a different package's unit"
    );
}

/// An independently named target must be declared by its own name.
///
/// A package `adapter` may declare `[lib] name = "engine"`, and Cargo then
/// files the output under the *target* name while the records keep the
/// *package* name. Declaring only `adapter` therefore owns
/// `.fingerprint/adapter-<hash>/` and not `deps/libengine-<hash>.rlib`, so
/// removing just the output leaves the recorded-name check satisfied while the
/// unit it needs is gone. Both halves are pinned as separate cases: the package
/// name alone is not enough, and the pair owns every shape Cargo writes.
#[rstest]
#[case::package_name_owns_the_records(
    &["adapter"],
    ".fingerprint/adapter-0123456789abcdef/invoked.timestamp",
    true
)]
#[case::package_name_does_not_own_the_output(
    &["adapter"],
    "deps/libengine-0123456789abcdef.rlib",
    false
)]
#[case::a_declared_target_owns_its_library(
    &["adapter", "engine"],
    "deps/libengine-0123456789abcdef.rlib",
    true
)]
#[case::a_declared_target_owns_its_metadata(
    &["adapter", "engine"],
    "deps/libengine-0123456789abcdef.rmeta",
    true
)]
#[case::a_declared_target_owns_its_dep_info(
    &["adapter", "engine"],
    "deps/engine-0123456789abcdef.d",
    true
)]
fn an_independently_named_target_needs_its_own_entry(
    #[case] declared: &[&str],
    #[case] name: &str,
    #[case] expected: bool,
) {
    assert_eq!(
        name_is_owned(name, &owned(declared)),
        expected,
        "`{name}` against the declaration {declared:?}"
    );
}

/// A family that names no package owns the whole profile.
///
/// [`super::super::super::Preparation::for_fixture`] sets `CARGO_TARGET_DIR`
/// to the directory it stamps, so nothing else writes there and there is
/// nothing to attribute.
#[test]
fn an_empty_ownership_claims_every_name() {
    assert!(name_is_owned("deps/libunit-0123456789abcdef.rlib", &[]));
}

/// The metadata suffix is whatever Cargo's hash is, and nothing else.
///
/// A stem that happens to end in hex digits is only split when exactly sixteen
/// of them follow the final `-`, which is what keeps a crate whose name ends in
/// something hash-shaped from being truncated into a different package. The
/// boundary cases — fifteen digits, seventeen digits, a non-hex character — are
/// their own cases rather than a run of assertions, so a failure names which
/// side of the boundary moved.
#[rstest]
#[case::bare_stem("libunit-0123456789abcdef", "libunit")]
#[case::with_extension("libunit-0123456789abcdef.rlib", "libunit")]
#[case::one_digit_short("unit-0123456789abcde", "unit-0123456789abcde")]
#[case::one_digit_long("unit-0123456789abcdef0", "unit-0123456789abcdef0")]
#[case::not_a_hash("unit-not-a-hash", "unit-not-a-hash")]
#[case::a_plain_file_name("invoked.timestamp", "invoked")]
fn only_a_real_metadata_hash_is_stripped(#[case] name: &str, #[case] expected: &str) {
    assert_eq!(strip_unit_suffix(name), expected);
}

/// The hash shape is sixteen hexadecimal characters and nothing else.
#[rstest]
#[case::lowercase("0123456789abcdef", true)]
#[case::uppercase("0123456789ABCDEF", true)]
#[case::one_short("0123456789abcde", false)]
#[case::one_long("0123456789abcdef0", false)]
#[case::a_non_hex_digit("0123456789abcdeg", false)]
fn the_hash_shape_is_sixteen_hexadecimal_characters(#[case] value: &str, #[case] expected: bool) {
    assert_eq!(is_metadata_hash(value), expected, "{value}");
}
