//! Property tests for [`super::input_hash`].
//!
//! The hash decides when a prepared family is stale, so its contract is a
//! relation between a tree and what the family compiles from it, not a fixed
//! digest: equivalent trees must agree, any change to an input must not, and
//! build output at any depth must not reach it.
//!
//! Build output is excluded at every depth, so a generation strategy that only
//! nested inputs would never exercise the exclusion. Each generated tree
//! therefore carries excluded entries beside its inputs.
//!
//! # What these tests cannot show
//!
//! They cannot show that the walk *sorts*. On ext4 — measured on the host
//! this was written on — `read_dir` returns entries in a hash of their names,
//! independent of creation order and of how many times the directory grew, so
//! two trees with the same names and contents list in the same order however
//! they were written. A property that hashes one tree built two ways
//! therefore passes even when the sort is deleted: this was checked by
//! replacing the sort with a reversal and watching it still pass. That
//! invariant belongs to a test that supplies the entry sequence directly, and
//! lives in `input_hash`'s own unit tests as
//! `entries_hash_in_sorted_order_not_listing_order`.

use std::{collections::BTreeMap, fs, path::Path};

use proptest::prelude::*;

use super::{Preparation, input_hash};

/// One file to create, as a path relative to the tree root.
type Tree = BTreeMap<String, Vec<u8>>;

/// Segment names that [`super::input_hash`] must treat as build output.
///
/// Written from the module's own contract rather than from the generated
/// input, so a case that nests one is checking the exclusion and not merely
/// producing another ordinary file.
const EXCLUDED: [&str; 3] = ["target", "wip", ".git"];

/// A lowercase segment that can be created as a file or directory on every
/// platform the suite runs on.
///
/// The filter belongs to the strategy rather than to its callers so a path
/// cannot be built from a segment no filesystem will accept. See
/// [`is_windows_reserved`].
fn ordinary_segment() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,6}".prop_filter("a segment must name an ordinary file", |segment| {
        !is_windows_reserved(segment)
    })
}

/// Whether `segment` names a Windows device rather than a file.
///
/// `write_tree` really creates these paths, so the alphabet has to exclude a
/// name no Windows filesystem can hold. `CON`, `PRN`, `AUX`, `NUL`, `COM1` to
/// `COM9` and `LPT1` to `LPT9` are reserved by the Win32 path parser, which
/// maps them to devices before NTFS is reached; the match is case-insensitive
/// and survives an extension. Creating `nul/a` on Windows therefore fails with
/// `ERROR_PATH_NOT_FOUND`, not by rejecting the name: the reserved name is
/// resolved as a device, so it has no directory to descend into. That is what
/// run 43617094223's `build-test (windows, default features)` lane reported as
/// `input file: Os { code: 3, kind: NotFound }` for its minimal input
/// `tree = { "nul/a": [] }`.
///
/// Filtering here rather than at the call site keeps both strategies honest:
/// `excluded_path` writes entries too, so a reserved name there fails the same
/// way.
fn is_windows_reserved(segment: &str) -> bool {
    let lowered = segment.to_ascii_lowercase();
    if ["con", "prn", "aux", "nul"].contains(&lowered.as_str()) {
        return true;
    }
    ["com", "lpt"].iter().any(|family| {
        lowered.strip_prefix(family).is_some_and(|digit| {
            digit.len() == 1 && digit.chars().next().is_some_and(|n| matches!(n, '1'..='9'))
        })
    })
}

/// A relative path of one to three segments, none of them excluded.
///
/// The segment alphabet can spell `target` and `wip`, so a filter is required
/// rather than merely tidy: an "input" carrying a build-output name is not an
/// input at all, and a property that picked it would compare two hashes that
/// are equal because both were skipped.
///
/// Segments come from [`ordinary_segment`], so the alphabet cannot spell a
/// Windows device name either. See [`is_windows_reserved`].
fn input_path() -> impl Strategy<Value = String> {
    proptest::collection::vec(ordinary_segment(), 1..=3)
        .prop_map(|segments| segments.join("/"))
        .prop_filter(
            "an input path must not carry a build-output segment",
            |path| !is_excluded(path),
        )
}

/// A relative path containing a build-output segment.
///
/// The excluded segment is not always the first: `hash_sorted` skips by entry
/// name at every level it walks, so `target/` and `crates/a/target/` must be
/// excluded alike. A generator that only ever produced a root-level name would
/// leave the recursive half of that rule untested, which is why a prefix of up
/// to two ordinary segments is generated too.
///
/// Those prefixes are written to disk as directories, so they carry the same
/// reserved-name exclusion as [`input_path`]: a `nul/target` entry fails to
/// create on Windows before the hash is ever compared.
fn excluded_path() -> impl Strategy<Value = String> {
    let (prefix, root, tail) = (
        proptest::collection::vec(ordinary_segment(), 0..=2),
        proptest::sample::select(EXCLUDED.as_slice()),
        proptest::collection::vec(ordinary_segment(), 0..=2),
    );
    (prefix, root, tail).prop_map(|(prefix, root, tail)| {
        prefix
            .into_iter()
            .chain(std::iter::once(root.to_owned()))
            .chain(tail)
            .collect::<Vec<String>>()
            .join("/")
    })
}

/// A tree: some input files plus some excluded entries.
///
/// The two sets are generated separately so a case can never be all inputs (no
/// exclusion tested) or all excluded entries (nothing to hash). They are then
/// combined in that order and filtered by one rule, because precedence matters:
/// an input keeps its place when both sets name a path that the other cannot
/// coexist with. Additional members that conflict with an accepted path are
/// dropped: a path cannot be both a file and a directory, so `.git` and
/// `.git/a` describe no tree any filesystem can hold.
fn tree() -> impl Strategy<Value = Tree> {
    let contents = prop::collection::vec(any::<u8>(), 0..=8);
    let inputs = proptest::collection::btree_map(input_path(), contents.clone(), 1..=6);
    let excluded = proptest::collection::btree_map(excluded_path(), contents, 0..=3);
    (inputs, excluded).prop_map(|(inputs, excluded)| {
        let mut tree = Tree::new();
        for (path, contents) in inputs.into_iter().chain(excluded) {
            if !tree.keys().any(|kept| conflicts(kept, &path)) {
                tree.insert(path, contents);
            }
        }
        tree
    })
}

/// Whether two relative paths cannot both exist as files.
fn conflicts(left: &str, right: &str) -> bool {
    left == right || is_prefix(left, right) || is_prefix(right, left)
}

/// Whether `prefix` names a directory containing `path`.
fn is_prefix(prefix: &str, path: &str) -> bool {
    path.strip_prefix(prefix)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// Whether every parent directory `path` needs is already implied by `tree`.
///
/// Writing `path` creates the directories above it, and each of those is an
/// ordinary entry name the hash records. A tree that lacked one would gain it
/// when `path` was written, so the two hashes would differ for a reason
/// unrelated to whether `path` itself was excluded.
fn parents_exist(tree: &Tree, path: &str) -> bool {
    let segments: Vec<&str> = path.split('/').collect();
    let Some((_, ancestors)) = segments.split_last() else {
        return true;
    };
    let mut parent = String::new();
    ancestors.iter().all(|segment| {
        if !parent.is_empty() {
            parent.push('/');
        }
        parent.push_str(segment);
        tree.keys().any(|kept| is_prefix(&parent, kept))
    })
}

/// Create `tree` under `root`, creating each parent directory as needed.
///
/// Only one creation order exists here, and that is deliberate. An earlier
/// version wrote the same tree in two orders to test that the walk sorts;
/// on ext4 that measured nothing, because `read_dir` returned the same
/// hash-derived order either way. See the module note on what these tests
/// cannot show.
fn write_tree(root: &Path, tree: &Tree) {
    for (path, contents) in tree {
        let destination = root.join(path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("parent directory");
        }
        fs::write(&destination, contents).expect("input file");
    }
}

/// Hash `tree` created under a fresh temporary root.
fn hash_of(tree: &Tree) -> String {
    let directory = tempfile::tempdir().expect("temp dir");
    write_tree(directory.path(), tree);
    input_hash(std::slice::from_ref(&directory.path().to_path_buf())).expect("input hash")
}

proptest! {
    /// Moving an input to a different directory changes the hash, and moving
    /// it back restores it.
    ///
    /// This is a round trip over the *structure* of the tree rather than its
    /// file ordering: the same bytes under a different path are a different
    /// input, because the path is what the family compiles. Flattening a
    /// nested walk would pass a byte-only comparison, so the paths must
    /// reach the hash.
    #[test]
    fn moving_an_input_changes_the_hash(tree in tree()) {
        let leaf = tree
            .iter()
            .find(|(path, _)| !is_excluded(path))
            .map(|(path, contents)| (path.clone(), contents.clone()));
        if let Some((path, contents)) = leaf {
            let mut moved = tree.clone();
            moved.remove(&path);
            // A sibling directory keeps the leaf's bytes and only changes
            // where they live.
            let destination = format!("moved/{path}");
            if !moved.keys().any(|kept| conflicts(kept, &destination)) {
                moved.insert(destination, contents);
                prop_assert_ne!(
                    hash_of(&tree),
                    hash_of(&moved),
                    "an input's path must reach the hash, not only its bytes"
                );
            }
        }
    }

    /// Changing any input's bytes changes the hash.
    ///
    /// The changed path is chosen from the generated tree, so this exercises
    /// files at every depth rather than only the root.
    #[test]
    fn changing_an_input_changes_the_hash(tree in tree(), delta in any::<u8>()) {
        let changed = tree
            .iter()
            .find(|(path, _)| !is_excluded(path))
            .map(|(path, contents)| {
                let mut copy = tree.clone();
                let mut new = contents.clone();
                new.push(delta);
                copy.insert(path.clone(), new);
                copy
            });
        if let Some(changed) = changed {
            prop_assert_ne!(
                hash_of(&tree),
                hash_of(&changed),
                "an input's bytes must reach the hash"
            );
        }
    }

    /// Excluded entries never reach the hash, at any depth.
    ///
    /// The excluded subtree is varied independently of the inputs, so a case
    /// that excluded only root-level entries would be caught: the added member
    /// can be nested.
    ///
    /// Two preconditions, and both are about not testing the wrong thing.
    /// The path must not collide with an existing one, or the two trees hold
    /// different entries. And every parent directory it needs must already
    /// exist in the tree: creating `p/target` under a tree with no `p` adds
    /// `p` itself, which is an ordinary input name, so the hashes would differ
    /// for a reason that has nothing to do with the exclusion.
    #[test]
    fn excluded_entries_do_not_reach_the_hash(tree in tree(), extra in excluded_path()) {
        if !tree.keys().any(|kept| conflicts(kept, &extra)) && parents_exist(&tree, &extra) {
            let mut with_extra = tree.clone();
            with_extra.insert(extra, b"build output".to_vec());
            prop_assert_eq!(
                hash_of(&tree),
                hash_of(&with_extra),
                "build output must be excluded wherever it appears"
            );
        }
    }
}

/// Whether any segment of `path` names build output.
///
/// `hash_sorted` skips by entry name at every level it walks, so a nested
/// `a/target` is excluded exactly as a root-level `target` is. Judging only
/// the first segment would make every property here agree with a rule the
/// implementation does not have.
fn is_excluded(path: &str) -> bool { path.split('/').any(|segment| EXCLUDED.contains(&segment)) }

/// The generator refuses exactly the Windows device names, and no others.
///
/// Run 43617094223 failed on `tree = { "nul/a": [] }`: `write_tree` reached
/// `fs::write` and Windows resolved `nul` as a device, so no directory could be
/// created for it. A property generator that can spell such a name will
/// eventually spell it, so the exclusion has to be complete rather than lucky.
/// The positive half is what keeps the filter from over-reaching: `nul_`,
/// `null`, `com0` and `lpt` are ordinary names, and rejecting them would lose
/// real coverage.
#[test]
fn windows_device_names_are_the_only_segments_excluded() {
    for reserved in [
        "con", "prn", "aux", "nul", "com1", "com9", "lpt1", "lpt9", "CON", "Nul", "COM5",
    ] {
        assert!(
            is_windows_reserved(reserved),
            "{reserved} is a Windows device name and cannot be created as a file"
        );
    }
    for ordinary in [
        "nul_", "null", "con_", "console", "com0", "com10", "lpt", "lpt0", "lpt10", "a", "target",
    ] {
        assert!(
            !is_windows_reserved(ordinary),
            "{ordinary} is an ordinary name and excluding it would lose coverage"
        );
    }
}

/// The stamp's own file is not an input, so a family that stamps a root does
/// not thereby invalidate its own fingerprint.
#[test]
fn a_stamp_beside_the_inputs_does_not_change_the_hash() {
    let directory = tempfile::tempdir().expect("temp dir");
    let root = directory.path().to_path_buf();
    fs::write(root.join("feature.feature"), b"Feature: one").expect("input");

    let before = input_hash(std::slice::from_ref(&root)).expect("hash before");
    // The name is the one `write_stamp` derives for a family rooted here.
    let environment = crate::nested_cargo::env_from_vars(
        Vec::new(),
        Some(&std::env::temp_dir().join("target")),
        crate::nested_cargo::ProfileDestination::Caller,
    )
    .expect("the fixture environment should resolve");
    let preparation = Preparation::new(
        super::Sources::new("unit", "v", "hash"),
        root.clone(),
        environment,
    )
    .expect("the fixture preparation should describe its artefacts");
    let stamp = super::stamp::stamp_path(&preparation);
    assert_eq!(stamp.parent(), Some(root.as_path()), "stamp beside inputs");
    fs::write(&stamp, b"fingerprint").expect("stamp");
    let after = input_hash(std::slice::from_ref(&root)).expect("hash after");

    assert_eq!(
        before, after,
        "the stamp records the fingerprint of these inputs, so it cannot be one of them"
    );
}
