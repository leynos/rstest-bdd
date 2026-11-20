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

use super::input_hash;

/// One file to create, as a path relative to the tree root.
type Tree = BTreeMap<RelPath, Vec<u8>>;

/// A `/`-separated path inside a generated tree.
///
/// Every relation these properties assert — that one path is a prefix of
/// another, that two cannot coexist, that a path is build output — is a fact
/// about the *segments* of a tree path, not about a free string. Keeping them
/// on one type is what stops a caller pairing an arbitrary string with one
/// that names a tree entry: there is no way to spell a relation here without
/// first constructing the path it is about.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RelPath(String);

impl RelPath {
    /// Take ownership of an already-joined `/`-separated path.
    fn new(path: String) -> Self { Self(path) }

    /// The path as the `/`-joined string the filesystem call wants.
    fn as_str(&self) -> &str { &self.0 }

    /// Whether `self` names a directory containing `other`.
    fn is_prefix_of(&self, other: &Self) -> bool {
        other
            .as_str()
            .strip_prefix(self.as_str())
            .is_some_and(|rest| rest.starts_with('/'))
    }

    /// Whether two paths cannot both exist as files.
    fn conflicts_with(&self, other: &Self) -> bool {
        self == other || self.is_prefix_of(other) || other.is_prefix_of(self)
    }

    /// Whether any segment names build output.
    ///
    /// `hash_sorted` skips by entry name at every level it walks, so a nested
    /// `a/target` is excluded exactly as a root-level `target` is. Judging
    /// only the first segment would make every property here agree with a
    /// rule the implementation does not have.
    fn is_excluded(&self) -> bool { self.0.split('/').any(|segment| EXCLUDED.contains(&segment)) }
}

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
fn input_path() -> impl Strategy<Value = RelPath> {
    proptest::collection::vec(ordinary_segment(), 1..=3)
        .prop_map(|segments| RelPath::new(segments.join("/")))
        .prop_filter(
            "an input path must not carry a build-output segment",
            |path| !path.is_excluded(),
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
fn excluded_path() -> impl Strategy<Value = RelPath> {
    let (prefix, root, tail) = (
        proptest::collection::vec(ordinary_segment(), 0..=2),
        proptest::sample::select(EXCLUDED.as_slice()),
        proptest::collection::vec(ordinary_segment(), 0..=2),
    );
    (prefix, root, tail).prop_map(|(prefix, root, tail)| {
        RelPath::new(
            prefix
                .into_iter()
                .chain(std::iter::once(root.to_owned()))
                .chain(tail)
                .collect::<Vec<String>>()
                .join("/"),
        )
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
            if !tree.keys().any(|kept| kept.conflicts_with(&path)) {
                tree.insert(path, contents);
            }
        }
        tree
    })
}

/// Create `tree` under `root`, creating each parent directory as needed.
///
/// Only one creation order exists here, and that is deliberate. An earlier
/// version wrote the same tree in two orders to test that the walk sorts;
/// on ext4 that measured nothing, because `read_dir` returned the same
/// hash-derived order either way. See the module note on what these tests
/// cannot show.
fn write_tree(root: &Path, tree: &Tree) -> std::io::Result<()> {
    for (path, contents) in tree {
        let destination = root.join(path.as_str());
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&destination, contents)?;
    }
    Ok(())
}

/// Hash `tree` created under a fresh temporary root.
fn hash_of(tree: &Tree) -> std::io::Result<String> {
    let directory = tempfile::tempdir()?;
    write_tree(directory.path(), tree)?;
    input_hash(std::slice::from_ref(&directory.path().to_path_buf()))
}

/// Report a fixture setup failure as one, not as a failed hash relation.
///
/// Proptest has a single failure channel for a case, so the distinction has to
/// be carried by the message: a tree that could not be written must not read as
/// an input that wrongly reached, or wrongly missed, the hash.
fn setup<T>(result: std::io::Result<T>) -> Result<T, TestCaseError> {
    result.map_err(|error| {
        TestCaseError::fail(format!("the fixture tree could not be prepared: {error}"))
    })
}

proptest! {
    /// Renaming an input changes the hash, with its bytes and the rest of the
    /// tree held fixed.
    ///
    /// The input sits alone in a directory of its own, so the two trees differ
    /// only in that entry's name. Moving it into a new directory would change
    /// the tree's shape too, so a hash that ignored names could still differ.
    /// The bytes match under either name, so the path must reach the hash.
    #[test]
    fn renaming_an_input_changes_the_hash(tree in tree()) {
        let leaf = tree
            .iter()
            .find(|(path, _)| !path.is_excluded())
            .map(|(path, contents)| (path.clone(), contents.clone()));
        if let Some((path, contents)) = leaf {
            let directory = RelPath::new(String::from("dedicated"));
            let mut rest = tree.clone();
            rest.remove(&path);
            if !rest.keys().any(|kept| kept.conflicts_with(&directory)) {
                let name = path.as_str().rsplit('/').next().unwrap_or_default();
                let mut before = rest.clone();
                before.insert(RelPath::new(format!("dedicated/{name}")), contents.clone());
                let mut after = rest;
                after.insert(RelPath::new(format!("dedicated/{name}_")), contents);
                let before = setup(hash_of(&before))?;
                let after = setup(hash_of(&after))?;
                prop_assert_ne!(
                    before,
                    after,
                    "an input's name must reach the hash, not only its bytes"
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
            .find(|(path, _)| !path.is_excluded())
            .map(|(path, contents)| {
                let mut copy = tree.clone();
                let mut new = contents.clone();
                new.push(delta);
                copy.insert(path.clone(), new);
                copy
            });
        if let Some(changed) = changed {
            let before = setup(hash_of(&tree))?;
            let after = setup(hash_of(&changed))?;
            prop_assert_ne!(
                before,
                after,
                "an input's bytes must reach the hash"
            );
        }
    }

    /// An excluded entry at the root of the tree never reaches the hash.
    ///
    /// The entry is added beside the generated inputs, with a generated tail
    /// beneath it, so nothing above it has to exist. The one precondition is
    /// that it does not collide with a path the tree already holds, or the two
    /// trees would hold different entries. Every other case compares.
    #[test]
    fn excluded_entries_at_the_root_do_not_reach_the_hash(
        tree in tree(),
        root in proptest::sample::select(EXCLUDED.as_slice()),
        tail in proptest::collection::vec(ordinary_segment(), 1..=2),
    ) {
        let extra = RelPath::new(format!("{root}/{}", tail.join("/")));
        prop_assume!(!tree.keys().any(|kept| kept.conflicts_with(&extra)));
        let mut with_extra = tree.clone();
        with_extra.insert(extra, b"build output".to_vec());
        prop_assert_eq!(
            setup(hash_of(&tree))?,
            setup(hash_of(&with_extra))?,
            "build output at the root must be excluded"
        );
    }

    /// An excluded entry beside an input, inside a directory, never reaches
    /// the hash.
    ///
    /// The directory is built here, not hoped for: it holds an ordinary input
    /// in both trees, so adding `<directory>/<excluded>/output` creates no
    /// ordinary name, and the comparison is reached in every case. A generated
    /// prefix would exist in the tree only by coincidence, and a case whose
    /// prefix was absent passed without comparing anything.
    #[test]
    fn excluded_entries_beside_an_input_do_not_reach_the_hash(
        tree in tree(),
        root in proptest::sample::select(EXCLUDED.as_slice()),
    ) {
        let directory = RelPath::new(String::from("dedicated"));
        prop_assume!(!tree.keys().any(|kept| kept.conflicts_with(&directory)));
        let mut without = tree;
        without.insert(RelPath::new(String::from("dedicated/input")), b"input".to_vec());
        let mut with_extra = without.clone();
        let nested = RelPath::new(format!("dedicated/{root}/output"));
        with_extra.insert(nested, b"build output".to_vec());
        prop_assert_eq!(
            setup(hash_of(&without))?,
            setup(hash_of(&with_extra))?,
            "build output inside a directory must be excluded"
        );
    }
}

mod fixed_cases;
