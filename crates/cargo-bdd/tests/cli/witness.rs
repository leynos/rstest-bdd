//! The witness that preparation left the fixture's own test binaries.
//!
//! The timed CLI assertions do not care that a build ran: they inspect the
//! fixture's compiled test binaries. A preparation that exited zero while
//! leaving them out has not prepared this family, however many other units it
//! wrote, so the preparation test asserts on these files directly.
//!
//! The witness follows the profile rather than naming a directory, because
//! Cargo's build-directory layout is not a stable interface — see
//! [`fixture_binaries_present`].

use std::path::Path;

/// The fixture's own test binaries, as Cargo names them in `deps`.
///
/// Both files the fixture declares are listed: each is a timed CLI assertion's
/// subject, and each is a unit the preparation is supposed to leave behind.
///
/// Visible to the parent so its recovery case can write the binaries this
/// witness looks for. A second copy of the list there would let the two drift,
/// and a recovery case testing a stale copy would pass while the real family
/// stayed broken.
pub(super) const FIXTURE_TEST_BINARIES: [&str; 2] =
    ["diagnostics_fixture", "unsupported_dump_steps"];

/// Whether anything below `profile` is the fixture's own compiled test binary.
///
/// The search walks the profile rather than naming a directory. Cargo's
/// build-directory layout is explicitly not a stable interface and changed
/// under this suite once already: 1.100.0 moved every unit out of
/// `<profile>/deps` into `<profile>/build/<pkg>/<hash>/out`, which left a
/// witness that named `deps` reading a directory that no longer exists on
/// Windows CI. A witness that follows the profile works against both layouts
/// and against the next one.
///
/// Cargo suffixes every unit with a hash, so the name is matched on its stem.
/// A name alone cannot witness a binary, though, so two further rules apply:
/// the entry must be a file rather than a directory, and its extension must be
/// one an executable carries. See [`witnesses_binary`] for both.
///
/// Returns an error when `profile` itself cannot be listed at all, which the
/// caller reports as a failure rather than as an absent set: a target whose
/// directory cannot be read is not one that has been prepared, and the
/// distinction is worth keeping in the message.
pub(super) fn fixture_binaries_present(profile: &Path) -> std::io::Result<bool> {
    // `read_dir` on the profile is what reports an absent tree, so that error
    // survives even though the subdirectories below it are walked leniently.
    let root = std::fs::read_dir(profile)?;
    let mut pending: Vec<std::fs::DirEntry> = root.collect::<std::io::Result<_>>()?;
    let mut present = vec![false; FIXTURE_TEST_BINARIES.len()];
    while let Some(entry) = pending.pop() {
        let path = entry.path();
        // A subdirectory that vanishes between the listing and this read is
        // skipped: the profile is the thing under test, not the race.
        if let Ok(children) = std::fs::read_dir(&path) {
            pending.extend(children.filter_map(std::io::Result::ok));
        }
        // `fs::metadata` follows a symlink, so a linked binary still counts:
        // what makes an entry a candidate is being a file at that path, not
        // how the directory entry was recorded. A directory named like the
        // fixture's binary fails here, which is the point — its name is the
        // only thing it shares with one.
        let is_file = std::fs::metadata(&path).is_ok_and(|metadata| metadata.is_file());
        if is_file {
            note_binary(&entry.file_name().to_string_lossy(), &mut present);
        }
    }
    Ok(present.iter().all(|found| *found))
}

/// Record `name` against the fixture binary it witnesses, if it witnesses one.
///
/// Kept apart from the walk so each concern reads on its own: the walk answers
/// "what entries are below the profile", and this answers "which of them count".
fn note_binary(name: &str, present: &mut [bool]) {
    for (index, stem) in FIXTURE_TEST_BINARIES.iter().enumerate() {
        let Some(found) = present.get_mut(index) else {
            continue;
        };
        if witnesses_binary(name, stem) {
            *found = true;
        }
    }
}

/// The extensions that *are* a test binary, lowercased.
///
/// An executable carries no extension at all on Unix and `.exe` on Windows, so
/// this is the whole list. Everything else sharing the stem is a **sidecar**
/// written beside the binary rather than the binary: `.d` dep-info on every
/// platform, `.pdb` on MSVC, `.dwp` on a split-dwarf Linux build, `.rmeta` for
/// a metadata-only unit. An allowlist is the right shape because the rule that
/// makes a name a witness is "this is an executable", and a denylist of known
/// sidecars would silently accept the next one rustc invents.
const EXECUTABLE_EXTENSIONS: [&str; 1] = ["exe"];

/// Whether `name` is the compiled test binary for the fixture `stem`.
///
/// Cargo writes `<stem>-<hash>` for the binary, `<stem>-<hash>.d` for its
/// dep-info record, and on some targets `<stem>-<hash>.pdb` or
/// `<stem>-<hash>.dwp` beside it. Requiring the `-` separator keeps a stem from
/// matching a longer sibling that merely begins with the same characters, and
/// the extension decides the rest: only an executable shape is a witness, so a
/// sidecar beside the file is not.
///
/// Extensions are compared case-insensitively, because the filesystems Cargo
/// targets do not agree on case and a case-sensitive test would be the wrong
/// question.
///
/// A name is not by itself enough either: a *directory* can carry the same
/// name, which is why [`fixture_binaries_present`] checks the entry's type
/// before calling this.
fn witnesses_binary(name: &str, stem: &str) -> bool {
    let Some(rest) = name.strip_prefix(stem) else {
        return false;
    };
    if !rest.starts_with('-') {
        return false;
    }
    std::path::Path::new(name)
        .extension()
        .is_none_or(|extension| {
            EXECUTABLE_EXTENSIONS
                .iter()
                .any(|allowed| extension.eq_ignore_ascii_case(allowed))
        })
}

#[cfg(test)]
mod tests {
    //! The witness's own regression cases.
    //!
    //! A check that only ever runs against a warm tree cannot distinguish a
    //! real preparation from a no-op, so each rejection is exercised against
    //! output that compiled nothing, and one case is exercised against the
    //! build-directory layout that moved the units.

    use std::{fs, io, path::Path};

    use super::{FIXTURE_TEST_BINARIES, fixture_binaries_present};

    /// Write the file names Cargo would leave for one unit.
    ///
    /// Both halves are written, because a real tree carries the dep-info record
    /// and the output it describes. The hash suffix is what Cargo adds, so the
    /// names here are the shape the witness has to read.
    fn write_unit(deps: &Path, stem: &str, hash: &str, output: Option<&str>) {
        fs::write(deps.join(format!("{stem}-{hash}.d")), b"dep-info").expect("dep-info");
        if let Some(output) = output {
            fs::write(deps.join(output), b"unit").expect("unit output");
        }
    }

    #[test]
    fn a_tree_of_unrelated_artefacts_is_not_a_prepared_fixture() {
        // The no-op case the preparation test must reject. Every directory the
        // stamp's structural layer looks for is present and populated, so a
        // check that answered "the profile is not empty" would accept it; only a
        // witness naming the fixture's own units can tell the two apart.
        let scratch = tempfile::tempdir().expect("temp dir");
        let deps = scratch.path().join("debug").join("deps");
        fs::create_dir_all(&deps).expect("deps dir");
        fs::create_dir_all(scratch.path().join("debug").join(".fingerprint"))
            .expect("fingerprint dir");
        fs::write(
            scratch
                .path()
                .join("debug")
                .join(".fingerprint")
                .join("unit"),
            b"record",
        )
        .expect("fingerprint record");
        write_unit(
            &deps,
            "libunrelated",
            "0123456789abcdef",
            Some("libunrelated-0123456789abcdef.rlib"),
        );
        // Not even one of the fixture's, let alone both.
        write_unit(
            &deps,
            FIXTURE_TEST_BINARIES[0],
            "0123456789abcdef",
            Some("diagnostics_fixture-0123456789abcdef"),
        );

        let present = fixture_binaries_present(&deps).expect("the fixture deps should list");
        assert!(
            !present,
            "a tree holding only one of the fixture's binaries must not be reported as prepared"
        );
    }

    #[test]
    fn the_fixture_binaries_are_recognized_when_present() {
        // The positive control. Without it the test above would pass for a
        // witness that always answered "no". The output is named the way Cargo
        // names a test binary — `<stem>-<hash>` — rather than as a bare stem,
        // because the bare stem is the shape the witness must *not* accept: a
        // dep-info record alone would otherwise witness a unit with no binary.
        let scratch = tempfile::tempdir().expect("temp dir");
        let deps = scratch.path().join("debug").join("deps");
        fs::create_dir_all(&deps).expect("deps dir");
        for stem in FIXTURE_TEST_BINARIES {
            let binary = format!("{stem}-0123456789abcdef");
            write_unit(&deps, stem, "0123456789abcdef", Some(&binary));
        }

        let present = fixture_binaries_present(&deps).expect("the fixture deps should list");
        assert!(
            present,
            "the witness must recognize both fixture binaries in {}",
            deps.display()
        );
    }

    #[test]
    fn a_dep_info_record_alone_does_not_witness_a_binary() {
        // The narrowest no-op: the record is there, the binary it describes is
        // not. Reading the record as the witness would accept a tree Cargo
        // itself would recompile, which is the whole state the preparation
        // exists to exclude.
        let scratch = tempfile::tempdir().expect("temp dir");
        let deps = scratch.path().join("debug").join("deps");
        fs::create_dir_all(&deps).expect("deps dir");
        for stem in FIXTURE_TEST_BINARIES {
            write_unit(&deps, stem, "0123456789abcdef", None);
        }

        let present = fixture_binaries_present(&deps).expect("the fixture deps should list");
        assert!(
            !present,
            "a dep-info record without its binary must not be reported as prepared"
        );
    }

    #[test]
    fn a_directory_named_like_a_binary_does_not_witness_one() {
        // The name is the only thing a directory of this shape shares with a
        // test binary, and the walk visits directories as well as files, so a
        // witness that matched on name alone would accept a tree that holds no
        // executable at all. Both stems are given a directory, so the failure
        // cannot come from one of them merely being present.
        let scratch = tempfile::tempdir().expect("temp dir");
        let deps = scratch.path().join("debug").join("deps");
        fs::create_dir_all(&deps).expect("deps dir");
        for stem in FIXTURE_TEST_BINARIES {
            let impostor = deps.join(format!("{stem}-0123456789abcdef"));
            fs::create_dir_all(&impostor).expect("impostor dir");
            fs::write(impostor.join("inside"), b"not a binary").expect("contents");
        }

        let present = fixture_binaries_present(&deps).expect("the fixture deps should list");
        assert!(!present, "a directory named like a test binary is not one");
    }

    #[test]
    fn a_sidecar_beside_a_binary_is_not_itself_a_witness() {
        // MSVC writes `<stem>-<hash>.pdb` beside the executable, and a
        // split-dwarf Linux build writes `.dwp`. Both carry the binary's full
        // stem, so an extension-blind check reads them as the binary and
        // accepts a tree from which the executable has been deleted — the
        // exact state that sends a timed assertion into a rebuild.
        let scratch = tempfile::tempdir().expect("temp dir");
        let deps = scratch.path().join("debug").join("deps");
        fs::create_dir_all(&deps).expect("deps dir");
        for stem in FIXTURE_TEST_BINARIES {
            for extension in ["pdb", "dwp", "d", "rmeta"] {
                fs::write(
                    deps.join(format!("{stem}-0123456789abcdef.{extension}")),
                    b"sidecar",
                )
                .expect("sidecar");
            }
        }

        let present = fixture_binaries_present(&deps).expect("the fixture deps should list");
        assert!(
            !present,
            "only an executable witnesses a test binary, not a sidecar beside it"
        );
    }

    #[test]
    fn the_windows_executable_extension_witnesses_a_binary() {
        // The positive control for the extension rule, and the reason it is an
        // allowlist rather than a denylist: on Windows every test binary is
        // `<stem>-<hash>.exe`, so a check that demanded no extension would
        // report every prepared Windows tree as unprepared. `.EXE` is used
        // deliberately — the filesystem that produced it is case-insensitive,
        // and the comparison has to be too.
        let scratch = tempfile::tempdir().expect("temp dir");
        let deps = scratch.path().join("debug").join("deps");
        fs::create_dir_all(&deps).expect("deps dir");
        for stem in FIXTURE_TEST_BINARIES {
            fs::write(deps.join(format!("{stem}-0123456789abcdef.EXE")), b"binary")
                .expect("binary");
        }

        let present = fixture_binaries_present(&deps).expect("the fixture deps should list");
        assert!(present, "a Windows test binary must witness itself");
    }

    #[test]
    fn a_missing_profile_is_not_a_prepared_fixture() {
        // A target that was never built at all, rather than one built by
        // something else. The witness reports the unreadable directory as an
        // error: a tree whose profile cannot be listed is not one that has been
        // prepared, and reporting it as an empty set would lose that.
        let scratch = tempfile::tempdir().expect("temp dir");

        let error = fixture_binaries_present(&scratch.path().join("debug"))
            .expect_err("an absent profile must not be reported as prepared");
        assert_eq!(error.kind(), io::ErrorKind::NotFound, "{error}");
    }

    #[test]
    fn the_witness_finds_binaries_under_the_build_dir_layout() {
        // Cargo 1.100.0 moved every unit out of `<profile>/deps` into
        // `<profile>/build/<pkg>/<hash>/out` and left `deps` absent. The
        // witness named `deps`, so on Windows CI it read a directory that no
        // longer existed and every preparation of this family failed. The walk
        // must find the binaries wherever Cargo put them.
        let scratch = tempfile::tempdir().expect("temp dir");
        let out = scratch
            .path()
            .join("debug")
            .join("build/cargo-bdd/0123456789abcdef/out");
        fs::create_dir_all(&out).expect("unit dir");
        for stem in FIXTURE_TEST_BINARIES {
            write_unit(
                &out,
                stem,
                "0123456789abcdef",
                Some(&format!("{stem}-0123456789abcdef")),
            );
        }
        assert!(
            !scratch.path().join("debug").join("deps").exists(),
            "the fixture must model the layout that has no `deps` at all"
        );

        let present = fixture_binaries_present(&scratch.path().join("debug"))
            .expect("the profile should list");
        assert!(
            present,
            "the witness must find the binaries below the profile, not only in `deps`"
        );
    }
}
