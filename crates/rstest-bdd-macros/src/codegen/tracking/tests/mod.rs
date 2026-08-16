//! Unit tests for the Cargo rebuild-dependency tracking binding
//! (`ExecPlan` milestones 1d and 2).
//!
//! The token-shape tests pin the *exact* emitted tokens, so a future change
//! that keeps a substring-like resemblance but breaks the contract — for
//! example an absolute path smuggled into the literal, or a wrong relative
//! offset — fails loudly. Substring presence is not enough:
//! `include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", "the/wrong/file.feature"))`
//! would satisfy a contains-guard.
//!
//! The table tests pin the normalization rules from Table 1 of the
//! `ExecPlan`,
//! including the deliberate asymmetries: `..` segments are retained (they are
//! legal in `include_bytes!` and collapsing them could name a different file
//! through a symlink), `.` segments are dropped, the backslash rule is
//! Windows-only, and an absolute path outside `CARGO_MANIFEST_DIR`'s subtree
//! becomes a component-wise `..` offset.

use proptest::prelude::*;
use rstest::rstest;

use super::{TrackedFeaturePath, Untrackable, feature_tracking_item};
use quote::quote;

/// The doc text the tracking `const` must carry, rebuilt here as a
/// `concat!` too so the exact-equality assertion cannot be vacuous while
/// remaining independent of the implementation's wording.
const EXPECTED_BINDING_DOC: &str = concat!(
    "Registers the bound `.feature` file as a Cargo rebuild dependency (ADR-010). ",
    "Deleting this makes `.feature`-only edits silently skip recompilation; see ",
    "rstest-bdd::feature_rebuild_invalidation.",
);

/// The exact binding the tracking mechanism must emit, rebuilt here
/// independently of the implementation so the assertion cannot be vacuous.
fn expected_binding(rel: &str) -> String {
    let rel_lit = syn::LitStr::new(rel, proc_macro2::Span::call_site());
    let tokens = quote! {
        #[doc = #EXPECTED_BINDING_DOC]
        const _: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", #rel_lit));
    };
    tokens.to_string()
}

#[test]
fn emits_exact_binding_for_representative_relative_path() {
    let emitted = feature_tracking_item(
        std::path::Path::new("tests/features/invalidation.feature"),
        proc_macro2::Span::call_site(),
    );
    assert_eq!(
        emitted.to_string(),
        expected_binding("tests/features/invalidation.feature"),
        "the tracking item must emit the exact deferred-path binding for a \
         relative input path"
    );
}

#[test]
fn snapshot_of_normalized_token_stream() {
    // `TokenStream::to_string()` prints no span information, so the snapshot
    // is a clean whole-text pin of the emitted form; a meaning change fails
    // loudly even where a single substring comparison could drift.
    insta::assert_snapshot!(
        feature_tracking_item(
            std::path::Path::new("tests/features/invalidation.feature"),
            proc_macro2::Span::call_site(),
        )
        .to_string()
    );
}

// ---------------------------------------------------------------------------
// Table 1 rows (relative inputs) from the ExecPlan.
// ---------------------------------------------------------------------------

#[rstest]
#[case("tests/features/x.feature", "tests/features/x.feature")]
#[case("./tests/x.feature", "tests/x.feature")]
#[case("a/./b/x.feature", "a/b/x.feature")]
#[case("a/../b/x.feature", "a/../b/x.feature")]
fn relative_literal_follows_table(#[case] input: &str, #[case] expected: &str) {
    let tracked =
        TrackedFeaturePath::try_new(std::path::Path::new(input)).expect("relative is trackable");
    assert_eq!(tracked.relative_literal(), expected);
}

#[cfg(windows)]
#[test]
fn windows_separators_become_slashes() {
    let tracked = TrackedFeaturePath::try_new(std::path::Path::new(r"a\b\x.feature"))
        .expect("relative is trackable");
    assert_eq!(tracked.relative_literal(), "a/b/x.feature");
}

#[cfg(not(windows))]
#[test]
fn posix_backslash_is_an_ordinary_filename_character() {
    let tracked = TrackedFeaturePath::try_new(std::path::Path::new(r"a\b\x.feature"))
        .expect("relative is trackable");
    assert_eq!(tracked.relative_literal(), r"a\b\x.feature");
}

// ---------------------------------------------------------------------------
// Absolute inputs: the D4 component-wise `..` offset.
// ---------------------------------------------------------------------------

#[test]
fn absolute_under_manifest_emits_plain_relative_path() {
    let tracked = TrackedFeaturePath::try_new_from(
        std::path::Path::new("/repo/crates/my/tests/features/x.feature"),
        std::path::Path::new("/repo/crates/my"),
    )
    .expect("same root is trackable");
    assert_eq!(tracked.relative_literal(), "tests/features/x.feature");
}

#[test]
fn absolute_sibling_emits_dotdot_offset() {
    let tracked = TrackedFeaturePath::try_new_from(
        std::path::Path::new("/repo/shared/x.feature"),
        std::path::Path::new("/repo/crates/my"),
    )
    .expect("same root is trackable");
    assert_eq!(tracked.relative_literal(), "../../shared/x.feature");
}

#[test]
fn absolute_parent_emits_dotdot_only() {
    let tracked = TrackedFeaturePath::try_new_from(
        std::path::Path::new("/repo/x.feature"),
        std::path::Path::new("/repo/crates/my"),
    )
    .expect("same root is trackable");
    assert_eq!(tracked.relative_literal(), "../../x.feature");
}

#[cfg(windows)]
#[test]
fn different_drives_are_unrelatable() {
    let result = TrackedFeaturePath::try_new_from(
        std::path::Path::new(r"C:\repo\x.feature"),
        std::path::Path::new(r"D:\repo"),
    );
    assert!(matches!(result, Err(Untrackable::UnrelatableRoot(_))));
}

#[cfg(windows)]
#[test]
fn same_drive_different_case_annotations_relate() {
    let tracked = TrackedFeaturePath::try_new_from(
        std::path::Path::new(r"c:\Repo\x.feature"),
        std::path::Path::new(r"C:\repo"),
    )
    .expect("same drive is trackable");
    assert_eq!(tracked.relative_literal(), "Repo/x.feature");
}

#[cfg(unix)]
#[test]
fn non_utf8_component_is_untrackable() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let raw = OsString::from_vec(b"tests/\xff\xfe.feature".to_vec());
    let result = TrackedFeaturePath::try_new(std::path::Path::new(&raw));
    assert!(matches!(result, Err(Untrackable::NonUtf8(_))));
}

// ---------------------------------------------------------------------------
// Proptest: the emitted literal round-trips and never takes a path shape.
// ---------------------------------------------------------------------------

proptest! {
    /// For any sequence of relative components (each `..` or a plain name),
    /// the emitted literal reproduces the exact same component sequence and
    /// never begins with a separator or a drive prefix.
    #[test]
    fn relative_literal_round_trips(
        components in prop::collection::vec(
            prop_oneof![Just("..".to_owned()), "[a-zA-Z0-9_]{1,24}"],
            1..8,
        )
    ) {
        let merged = components.join("/");
        let tracked = TrackedFeaturePath::try_new(std::path::Path::new(&merged))
            .expect("relative path must be trackable");
        let literal = tracked.relative_literal();
        prop_assert_eq!(literal, merged);
        prop_assert!(!literal.starts_with('/'));
        prop_assert!(!literal.contains(':'));
    }

    /// A generated relative path is always trackable: with the table's
    /// generator every component is a name or `..`, neither of which can
    /// make the path absolute or empty.
    #[test]
    fn generated_relative_path_is_trackable(
        components in prop::collection::vec("[a-zA-Z0-9_]{1,24}", 1..8)
    ) {
        let merged = components.join("/");
        prop_assert!(TrackedFeaturePath::try_new(std::path::Path::new(&merged)).is_ok());
    }
}

// ---------------------------------------------------------------------------
// Empty and error-shape guard.
// ---------------------------------------------------------------------------

#[test]
fn empty_path_is_untrackable() {
    let result =
        TrackedFeaturePath::try_new_from(std::path::Path::new(""), std::path::Path::new("/"));
    assert!(matches!(result, Err(Untrackable::Empty)));
}

/// POSIX single-root semantics: any two absolute paths relate, so the
/// `../`-offset computation always succeeds on POSIX; a "different root" is a
/// Windows drive/UNC concept (pinned by `different_drives_are_unrelatable`).
#[cfg(not(windows))]
#[test]
fn posix_absolute_paths_always_relate() {
    let tracked = TrackedFeaturePath::try_new_from(
        std::path::Path::new("/other/base/x.feature"),
        std::path::Path::new("/repo/crates/my"),
    )
    .expect("POSIX shares one filesystem root");
    // `/repo/crates/my` needs three `..` steps to reach `/`, then follows
    // `other/base/x.feature`.
    assert_eq!(tracked.relative_literal(), "../../../other/base/x.feature");
}
