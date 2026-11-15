//! Unit tests for the sorted walk, which the property tests cannot see.

use std::{collections::hash_map::DefaultHasher, fs, hash::Hasher};

use super::{hash_sorted, input_hash};

/// The same entries hash the same whatever order the directory listed
/// them in.
///
/// This cannot be expressed by creating a tree two ways. On ext4 —
/// measured on this host — `read_dir` returns entries in a hash of their
/// names, independent of creation order and of how many times the
/// directory grew, so two trees with the same names and contents list
/// identically however they were written. A test written that way passes
/// even when the sort is removed, which was verified by replacing the
/// sort with a reversal and watching it still pass.
///
/// Handing the walk a known-unsorted sequence is the only way to see the
/// difference: this fails immediately if the sort goes away.
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
