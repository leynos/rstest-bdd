//! Walking the runner tree, and reporting what the walk could not read.
//!
//! Split from the parent module because the two concerns are separable and each
//! is easier to review alone: this file knows how to enumerate files, and the
//! parent knows which words are forbidden in them. The split is also forced by
//! `module_max_lines`, whose remedy is exactly this.
//!
//! Nothing here holds a forbidden token, so this file is itself scanned by the
//! check it supports. That is deliberate: the parent's own exemption covers
//! only the file that carries the token list, and widening it to a directory
//! would create a place for a real leak to hide.

use camino::Utf8Path;
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, DirEntry},
};

/// One walk of the runner tree: what was read, and what could not be.
///
/// Unreadable paths are carried out rather than swallowed. A walk that quietly
/// skipped a file would leave that file's leaks unreported, and the scan's
/// silence would then be indistinguishable from a clean tree — the failure
/// mode the check exists to catch. The completeness guard in
/// [`super::the_scan_finds_the_runner_tree`] pins the paths it names, one per
/// module rather than one per test file; this covers the remainder, including
/// any file added later.
pub(super) struct Scanned {
    /// `(relative path, contents)` for every file the walk read.
    pub(super) sources: Vec<(String, String)>,
    /// One message per path the walk could not read, in no particular order.
    pub(super) unreadable: Vec<String>,
}

/// Walk one root directory.
///
/// Split from [`super::runner_sources`] so the negative control below can drive
/// the walk at a path that cannot be opened, which is the only way to observe
/// that the `unreadable` path is reachable at all.
pub(super) fn scan_root(root: &Utf8Path) -> Scanned {
    let mut scanned = Scanned {
        sources: Vec::new(),
        unreadable: Vec::new(),
    };
    let Ok(directory) = Dir::open_ambient_dir(root, ambient_authority()) else {
        scanned
            .unreadable
            .push(format!("{root}: the root could not be opened"));
        return scanned;
    };
    collect(&directory, "", &mut scanned);
    scanned.sources.sort_by(|left, right| left.0.cmp(&right.0));
    scanned.unreadable.sort();
    scanned
}

/// Recursively collect `(relative path, contents)` for every `.rs` file under
/// `directory`, except the parent module's own file.
///
/// `prefix` is `directory`'s own path relative to the runner root, and is empty
/// at the root. Symlinked directories are not followed: `cap-std` opens
/// directories without traversing symlinks, so the scan cannot be redirected
/// outside the tree it was pointed at.
///
/// The per-entry work lives in [`visit`], which keeps this function's own
/// complexity to the listing plus the loop. The split is not cosmetic: this
/// function's control flow is the recursion, and mixing it with four failure
/// arms made a single function the hardest thing in the module to read.
fn collect(directory: &Dir, prefix: &str, scanned: &mut Scanned) {
    let entries = match directory.entries() {
        Ok(entries) => entries,
        Err(error) => {
            scanned
                .unreadable
                .push(format!("{prefix}: listing failed: {error}"));
            return;
        }
    };
    for entry in entries {
        visit(directory, prefix, entry, scanned);
    }
}

/// Handle one directory entry, recursing into a subdirectory.
///
/// Every failure path records a message rather than returning silently, so a
/// path the walk could not read is always visible in [`Scanned::unreadable`]
/// and can never be mistaken for a file with no leaks.
fn visit(
    directory: &Dir,
    prefix: &str,
    entry: Result<DirEntry, std::io::Error>,
    scanned: &mut Scanned,
) {
    let Ok(entry) = entry else {
        scanned
            .unreadable
            .push(format!("{prefix}: an entry could not be read"));
        return;
    };
    let Ok(name) = entry.file_name() else {
        scanned
            .unreadable
            .push(format!("{prefix}: an entry had no file name"));
        return;
    };
    let relative = child_path(prefix, &name);
    let Ok(file_type) = entry.file_type() else {
        scanned
            .unreadable
            .push(format!("{relative}: the file type could not be read"));
        return;
    };
    if file_type.is_dir() {
        match directory.open_dir(&name) {
            Ok(child) => collect(&child, &relative, scanned),
            Err(error) => scanned.unreadable.push(format!(
                "{relative}: the directory could not be opened: {error}"
            )),
        }
    } else if is_rust_source(&name) && relative != SELF {
        read_source(directory, &name, relative, scanned);
    }
}

/// Read one source file into `scanned`, or record why it could not be read.
///
/// Reading the token list's own source would find the literal tokens in it and
/// report them as leaks, which is why the caller checks `relative != SELF`
/// before getting here.
fn read_source(directory: &Dir, name: &str, relative: String, scanned: &mut Scanned) {
    match directory.read_to_string(name) {
        Ok(contents) => scanned.sources.push((relative, contents)),
        Err(error) => scanned
            .unreadable
            .push(format!("{relative}: the file could not be read: {error}")),
    }
}

/// The parent module's own file, whose contents the walk must not read.
///
/// The path relative to the runner root, not the bare file name. A bare name
/// would exempt *every* file called `surface.rs` anywhere in the tree, so a
/// nested `outcome/surface.rs` holding a frontend import would be skipped and
/// the scan would report a clean sweep of a file it never read — the exact
/// silence this module exists to prevent.
const SELF: &str = "tests/surface.rs";

/// Whether a directory entry is a Rust source file.
///
/// The comparison is case-insensitive, so a `.RS` file counts as source rather
/// than slipping past the scan. `Utf8Path::extension` only splits the name; it
/// does not fold case, so a bare `ext == "rs"` would let an upper-case spelling
/// of the same file through. A leak in such a file would then be reported as no
/// leak at all — the failure mode this whole check exists to catch, since its
/// silence is indistinguishable from a clean tree.
fn is_rust_source(name: &str) -> bool {
    Utf8Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("rs"))
}

/// Join a relative directory prefix to one of its children's names.
///
/// The root has an empty prefix, so its children are named bare; deeper entries
/// keep the whole path, which is what makes a nested finding identifiable.
fn child_path(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}/{name}")
    }
}

/// An unreadable path is reported rather than skipped.
///
/// This is the negative control for the `unreadable` arm: it drives the walk at
/// a root that cannot be opened, and requires that the walk says so. Without
/// it the arm could be dead code — an `unreadable` field that never fills — and
/// the parent's assertions that it is empty would pass for the wrong reason,
/// proving only that the field exists. A guard whose firing has never been
/// observed is indistinguishable from no guard, which is the same defect the
/// check itself exists to catch.
#[test]
fn an_unreadable_root_is_reported() {
    let missing = Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("src/runner/not-a-real-directory");
    let scanned = scan_root(&missing);

    assert!(
        scanned.sources.is_empty(),
        "a root that cannot be opened yields no sources",
    );
    assert_eq!(
        scanned.unreadable.len(),
        1,
        "the walk must report the root it could not open; got {:?}",
        scanned.unreadable,
    );
    let [message] = scanned.unreadable.as_slice() else {
        panic!("exactly one message was asserted above");
    };
    assert!(
        message.contains("could not be opened"),
        "the message must name the failure; got {message:?}",
    );
}

/// An upper-case extension still counts as a Rust source file.
///
/// `Utf8Path::extension` splits the name but does not fold case, so an
/// `ext == "rs"` test lets `thing.RS` past the scan entirely: the file is never
/// read, its leaks are never reported, and the scan's silence is
/// indistinguishable from a clean tree. This is the guard the earlier
/// case-sensitive comparison lacked.
#[test]
fn an_upper_case_extension_is_still_source() {
    assert!(is_rust_source("mod.rs"), "a plain .rs file must be source");
    assert!(
        is_rust_source("odd.RS"),
        "an upper-case .RS file must not slip past the scan",
    );
    assert!(
        is_rust_source("odd.Rs"),
        "a mixed-case .Rs file must not slip past the scan",
    );

    // The counterpart: folding case must not widen the scan to everything.
    // The messages avoid naming a document format, both because the extension
    // is all that matters and because this file is itself scanned by the check
    // it supports: a format name here would be reported as a leak.
    assert!(
        !is_rust_source("notes.md"),
        "a file with another extension is not source",
    );
    assert!(
        !is_rust_source("README"),
        "a file with no extension is not source",
    );
}

/// The walk actually reaches the tree the scan claims to police.
///
/// Split out of the parent module along with the walk it describes: the
/// assertion is about what [`scan_root`] returned, not about which words are
/// forbidden, so it belongs beside the walk rather than beside the token list.
///
/// Without this, a `collect` that silently returned nothing (a moved directory,
/// a changed extension) would make `no_frontend_types_in_public_api` pass while
/// checking nothing at all.
///
/// The expectations are whole relative paths, not bare file names. A name-only
/// expectation cannot tell a descent into `outcome/` from an unrelated
/// `outcome.rs` sitting at the top level: an earlier draft of this guard matched
/// `"outcome"` against each file name, and the literal was satisfied by
/// `tests/outcome.rs` — a file the guard was not asking about — so it proved
/// nothing about the directory it named. Whole paths leave no such slack. A
/// [`child_path`] that dropped its prefix, collapsing every key to a bare file
/// name, is caught here and slips past the name-only form.
///
/// The list went stale twice, and that is worth stating because each staleness
/// was invisible. It first named the files of EP-M1 and was not extended when
/// EP-M2 added `scope.rs` and the whole `engine/` subtree. It then named
/// `engine/drive_sync.rs` but not `engine/drive.rs` or `engine/drive_async.rs`,
/// which landed later, so it claimed a subtree it was only partly watching —
/// the one direction in which a completeness guard fails quietly.
///
/// Extending it again does not fix the recurrence, which is the list's real
/// defect: a hand-kept inventory of a growing tree drifts by construction. What
/// the entries are load-bearing *for* is this module's own correctness — the
/// prefixing `child_path` does, and the descent into each subdirectory — and
/// that is what the list is kept for. Per-module coverage within a proven
/// directory is claimed too, so the two driver entries are that claim being made
/// true again rather than a new proof about the walk.
///
/// `engine/policy_tests/` is named by its `mod.rs` for the reason above: one
/// entry that proves the *descent*, not one entry per test file.
#[test]
fn the_scan_finds_the_runner_tree() {
    let Scanned {
        sources,
        unreadable,
    } = super::runner_sources();
    assert!(
        unreadable.is_empty(),
        "the completeness guard must read the whole tree; unreadable:\n{}",
        unreadable.join("\n"),
    );
    let paths = sources
        .iter()
        .map(|(path, _)| path.as_str())
        .collect::<Vec<_>>();

    for expected in [
        "mod.rs",
        "engine/drive.rs",
        "engine/drive_async.rs",
        "engine/drive_sync.rs",
        "engine/mod.rs",
        "engine/policy.rs",
        "engine/policy_tests/mod.rs",
        "outcome/failure.rs",
        "outcome/mod.rs",
        "outcome/step.rs",
        "plan.rs",
        "plan/builder.rs",
        "scope.rs",
        "source.rs",
        // The INV-7 module is a directory: `source/mod.rs` proves the descent,
        // by the same reasoning as `engine/policy_tests/mod.rs` above.
        "tests/source/mod.rs",
        // This file and the parent are the scan's own two files, but only the
        // parent carries the token list, so only the parent is exempt from
        // being scanned. This one is scanned like any other.
        "tests/surface/walk.rs",
    ] {
        assert!(
            paths.contains(&expected),
            "expected the scan to reach {expected}; found {paths:?}",
        );
    }
}
