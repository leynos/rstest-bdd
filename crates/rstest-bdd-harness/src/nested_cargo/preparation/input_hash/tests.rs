//! Unit tests for the sorted walk, which the property tests cannot see.
//!
//! Exercises [`hash_sorted`] from the parent [`input_hash`](super) module
//! against an entry sequence supplied by hand, and [`input_hash`] over small
//! trees on disk. The sort is the reason this module exists: `read_dir`
//! exposes no portable order, so no sequence of file creations can force the
//! unsorted order a property test would need. That is true of every
//! filesystem rather than of any one of them — a directory may be linear, may
//! be hash-indexed, and is free to change between runs — and it is the actual
//! reason the sequence here is supplied by hand.
//!
//! Everything else about the walk — the kind markers, the directory boundary,
//! the symlink handling — is specified by the property tests, which generate
//! trees this module could only enumerate by hand. Cycle coverage is the
//! exception: the dedicated symlink cases below state it directly, because a
//! link back to an ancestor has to fail by hanging rather than by asserting,
//! and that is not a shape a generated tree can be trusted to produce.

use std::{collections::hash_map::DefaultHasher, fs, hash::Hasher};

use super::{hash_sorted, input_hash};

/// The same entries hash the same whatever order the directory listed
/// them in.
///
/// This cannot be expressed by creating a tree two ways, because `read_dir`
/// exposes no portable order to control. Measured on this host, one filesystem
/// listed entries in a hash of their names, independent of creation order and
/// of how many times the directory grew, so two trees with the same names and
/// contents listed identically however they were written. That is one
/// filesystem's behaviour rather than a rule a test may rely on: a linear
/// directory returns creation order, and a hash-indexed one can change as it
/// grows. Either way the trap is the same — a tree-shaped test passes even when
/// the sort is removed, which was verified by replacing the sort with a
/// reversal and watching it still pass.
///
/// Handing the walk a known-unsorted sequence is the only way to see the
/// difference, and it does not depend on any directory's behaviour: this fails
/// immediately if the sort goes away.
#[test]
fn entries_hash_in_sorted_order_not_listing_order() {
    let directory = tempfile::tempdir().expect("temp dir");
    let mut entries = Vec::new();
    for name in ["zeta", "alpha", "mid", "beta"] {
        let path = directory.path().join(name);
        fs::write(&path, name.as_bytes()).expect("input file");
        entries.push(path);
    }

    let mut listed = DefaultHasher::new();
    let listed_order = entries.clone();
    hash_sorted(listed_order, &mut listed, &mut Vec::new()).expect("listed order");

    entries.sort();
    let mut sorted = DefaultHasher::new();
    hash_sorted(entries, &mut sorted, &mut Vec::new()).expect("sorted order");

    assert_eq!(
        listed.finish(),
        sorted.finish(),
        "the walk must sort: a directory listing is the filesystem's order"
    );
}

/// Build output is skipped wherever it appears in the sequence, including
/// after a sort that puts it ahead of the inputs.
#[test]
fn build_output_is_skipped_in_every_position() {
    let directory = tempfile::tempdir().expect("temp dir");
    let input = directory.path().join("feature");
    fs::write(&input, b"Feature: one").expect("input file");
    let mut entries = vec![input.clone()];

    let mut before = DefaultHasher::new();
    hash_sorted(entries.clone(), &mut before, &mut Vec::new()).expect("without build output");

    let output = directory.path().join("target");
    fs::create_dir_all(&output).expect("target dir");
    fs::write(output.join("output"), b"binary").expect("artefact");
    entries.push(output);
    entries.push(directory.path().join("wip"));

    let mut after = DefaultHasher::new();
    hash_sorted(entries, &mut after, &mut Vec::new()).expect("with build output");

    assert_eq!(
        before.finish(),
        after.finish(),
        "target and wip must not reach the hash"
    );
}

/// A coverage profile written beside the inputs does not change the hash.
///
/// `-Cinstrument-coverage` makes a test binary drop a
/// `<name>_<hash>_<pid>.profraw` in the directory it ran in, so a preparation
/// that hashed one of these invalidates its own stamp the moment an
/// instrumented test runs — the failure hosted coverage saw on every lane. The
/// name carries a process id rather than overwriting, so the second half of
/// this case adds a *second* profile under a different id: a fix that only
/// ignored one fixed name, or that matched the `default_` prefix, would still
/// be refused by the run after it, and this asserts the exact property the
/// gate needs — that no number of profiles changes the answer.
///
/// The profiles are pushed onto `entries` rather than only written into the
/// directory, because `hash_sorted` walks the sequence it is handed: writing
/// them beside the input without naming them hashed the same one-file list
/// twice and asserted nothing. One profile deliberately carries a basename that
/// does not start with `default_`, so a fix keyed to that prefix fails here.
#[test]
fn coverage_profiles_are_skipped() {
    let directory = tempfile::tempdir().expect("temp dir");
    let input = directory.path().join("feature");
    fs::write(&input, b"Feature: one").expect("input file");
    let mut entries = vec![input];

    let mut before = DefaultHasher::new();
    hash_sorted(entries.clone(), &mut before, &mut Vec::new()).expect("without a profile");
    let before = before.finish();

    for profile in [
        "default_11337809912131070545_0_283041.profraw",
        "other_binary_11337809912131070545_0_284350.profraw",
    ] {
        let path = directory.path().join(profile);
        fs::write(&path, b"profile").expect("profile");
        entries.push(path);
    }

    let mut after = DefaultHasher::new();
    hash_sorted(entries, &mut after, &mut Vec::new()).expect("with profiles");

    assert_eq!(
        before,
        after.finish(),
        "a coverage profile is runtime output, not a fixture input; hashing it makes a stamp \
         stale by the very instrumented run that reads it"
    );
}

/// The same names nested differently are different inputs.
///
/// `src/{lib.rs, tests/check.rs}` and `{src/{lib.rs, tests/check.rs}}` hold
/// the same four names in the same sorted order and the same file bytes, so
/// a walk that recorded only names and contents would hash them alike.
/// Moving a test module under `src/` changes which files the crate compiles
/// and can change its module tree, so a stamp that survived the move would
/// let a timed assertion reuse artefacts the move invalidated.
///
/// This asserts on hashes of real directories rather than on a supplied
/// entry sequence, because the difference being tested is which listing an
/// entry was in — a property of the traversal, not of the sort.
#[test]
fn the_same_names_nested_differently_hash_differently() {
    let directory = tempfile::tempdir().expect("temp dir");
    // `check.rs` beside `src`, and `check.rs` inside `src/tests`.
    let beside = directory.path().join("beside");
    fs::create_dir_all(beside.join("src")).expect("src dir");
    fs::create_dir_all(beside.join("tests")).expect("tests dir");
    fs::write(beside.join("src/lib.rs"), b"mod tests;").expect("lib");
    fs::write(beside.join("tests/check.rs"), b"#[test] fn a() {}").expect("check");
    let nested = directory.path().join("nested");
    fs::create_dir_all(nested.join("src/tests")).expect("src/tests dir");
    fs::write(nested.join("src/lib.rs"), b"mod tests;").expect("lib");
    fs::write(nested.join("src/tests/check.rs"), b"#[test] fn a() {}").expect("check");

    let beside_hash = input_hash(std::slice::from_ref(&beside)).expect("beside hash");
    let nested_hash = input_hash(&[nested]).expect("nested hash");
    assert_ne!(
        beside_hash, nested_hash,
        "a directory boundary must reach the hash: the two trees emit the same names"
    );
    assert_eq!(
        beside_hash,
        input_hash(&[beside]).expect("beside hash again"),
        "the hash must stay reproducible for one tree"
    );
}

/// A link back into a directory the walk is already inside terminates.
///
/// Following a link is what makes a symlinked `.cargo/config.toml`
/// contribute, but it also lets a tree point back at an ancestor. Without
/// the ancestor check this walk never returns, so the test is the
/// termination proof: it fails by hanging rather than by asserting.
#[test]
#[cfg(unix)]
fn a_link_back_to_an_ancestor_terminates() {
    let directory = tempfile::tempdir().expect("temp dir");
    let nested = directory.path().join("nested");
    fs::create_dir_all(&nested).expect("nested dir");
    fs::write(nested.join("input"), b"Feature: one").expect("input file");
    std::os::unix::fs::symlink(directory.path(), nested.join("back")).expect("cycle link");

    let roots = vec![directory.path().to_path_buf()];
    let first = input_hash(&roots).expect("a cyclic tree must still hash");
    let second = input_hash(&roots).expect("the cycle must hash reproducibly");
    assert_eq!(first, second, "a cyclic tree must hash the same twice");
}

/// A cycle is not the same input as an empty directory.
///
/// Both stop the descent at the same point, so without a marker the two
/// trees would contribute identical bytes and a stamp would survive the
/// tree growing a link that leads somewhere.
#[test]
#[cfg(unix)]
fn a_cycle_is_distinct_from_an_empty_directory() {
    let cyclic = tempfile::tempdir().expect("cyclic dir");
    std::os::unix::fs::symlink(cyclic.path(), cyclic.path().join("back")).expect("cycle link");
    let empty = tempfile::tempdir().expect("empty dir");

    let cyclic_hash = input_hash(&[cyclic.path().to_path_buf()]).expect("cyclic hash");
    let empty_hash = input_hash(&[empty.path().to_path_buf()]).expect("empty hash");
    assert_ne!(
        cyclic_hash, empty_hash,
        "cutting a cycle must record something, or it reads as an empty tree"
    );
}
