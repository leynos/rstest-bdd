//! Unit tests for the ownership predicate.

use super::{is_metadata_hash, name_is_owned, strip_unit_suffix};

/// A package's directory form and its crate-name form are both its units.
///
/// Cargo hyphenates the package for `.fingerprint/<package>-<hash>/` and
/// underscores it for `deps/lib<stem>-<hash>.rlib`, so a rule that knew only
/// one spelling would leave half of a family's own units unrecorded — and the
/// unchecked half is where a `cargo clean -p` would go unnoticed.
#[test]
fn both_spellings_of_a_package_are_recognised() {
    let owned = ["rstest-bdd-feature-addition-fixture".to_owned()];
    for name in [
        ".fingerprint/rstest-bdd-feature-addition-fixture-0123456789abcdef/invoked.timestamp",
        "deps/librstest_bdd_feature_addition_fixture-0123456789abcdef.rlib",
        "deps/rstest_bdd_feature_addition_fixture-0123456789abcdef",
        "build/rstest-bdd-feature-addition-fixture-0123456789abcdef/out/build-script-build",
    ] {
        assert!(
            name_is_owned(name, &owned),
            "`{name}` belongs to the package"
        );
    }
}

/// A sibling fixture's units are not this family's to vouch for.
///
/// This is the regression: both scratch families build into the shared
/// `target/debug`, and the rebuild experiment runs `cargo clean -p` on its own
/// fixture before every baseline. A stamp that recorded the sibling's units was
/// refused for ever afterwards.
#[test]
fn a_sibling_packages_units_are_not_owned() {
    let owned = ["rstest-bdd-feature-addition-fixture".to_owned()];
    for name in [
        ".fingerprint/rstest-bdd-rebuild-invalidation-fixture-0123456789abcdef/\
         lib-rstest_bdd_rebuild_invalidation_fixture",
        "deps/librstest_bdd_rebuild_invalidation_fixture-0123456789abcdef.rlib",
        "deps/invalidation-0123456789abcdef",
    ] {
        assert!(
            !name_is_owned(name, &owned),
            "`{name}` belongs to the sibling, not to this family"
        );
    }
}

/// A name that merely shares a prefix or a substring is not the package.
///
/// `rstest-bdd` and `rstest-bdd-feature-addition-fixture` both build into the
/// shared profile, and the shorter name is a prefix of the longer one's crate
/// stem. A substring test would claim every one of the longer package's units
/// for the shorter package — and, worse, would claim a sibling's units for it.
#[test]
fn a_prefix_of_a_package_name_is_another_package() {
    let owned = ["rstest-bdd-feature-addition-fixture".to_owned()];
    for name in [
        "deps/librstest_bdd-0123456789abcdef.rlib",
        "deps/rstest_bdd-0123456789abcdef.d",
        "deps/librstest_bdd_macros-0123456789abcdef.rlib",
    ] {
        assert!(
            !name_is_owned(name, &owned),
            "`{name}` is a different package's unit"
        );
    }
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
/// something hash-shaped from being truncated into a different package.
#[test]
fn only_a_real_metadata_hash_is_stripped() {
    assert_eq!(strip_unit_suffix("libunit-0123456789abcdef"), "libunit");
    assert_eq!(
        strip_unit_suffix("libunit-0123456789abcdef.rlib"),
        "libunit"
    );
    assert_eq!(
        strip_unit_suffix("unit-0123456789abcde"),
        "unit-0123456789abcde"
    );
    assert_eq!(
        strip_unit_suffix("unit-0123456789abcdef0"),
        "unit-0123456789abcdef0"
    );
    assert_eq!(strip_unit_suffix("unit-not-a-hash"), "unit-not-a-hash");
    assert_eq!(strip_unit_suffix("invoked.timestamp"), "invoked");
}

/// The hash shape is sixteen hexadecimal characters and nothing else.
#[test]
fn the_hash_shape_is_sixteen_hexadecimal_characters() {
    assert!(is_metadata_hash("0123456789abcdef"));
    assert!(is_metadata_hash("0123456789ABCDEF"));
    assert!(!is_metadata_hash("0123456789abcde"));
    assert!(!is_metadata_hash("0123456789abcdef0"));
    assert!(!is_metadata_hash("0123456789abcdeg"));
}
