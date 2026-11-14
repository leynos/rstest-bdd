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
const FIXTURE_TEST_BINARIES: [&str; 2] = ["diagnostics_fixture", "unsupported_dump_steps"];

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
/// The `.d` record is excluded by name rather than by a `file_type` check: both
/// are regular files, and it is the record's *name* that makes it the wrong
/// witness. This is the witness that preparation did work — the timed CLI
/// assertions only pass because these binaries are there to be inspected.
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
        // A subdirectory that vanishes between the listing and this read is
        // skipped: the profile is the thing under test, not the race.
        if let Ok(children) = std::fs::read_dir(entry.path()) {
            pending.extend(children.filter_map(std::io::Result::ok));
        }
        note_binary(&entry.file_name().to_string_lossy(), &mut present);
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

/// Whether `name` is the compiled test binary for the fixture `stem`.
///
/// Cargo writes `<stem>-<hash>` for the binary and `<stem>-<hash>.d` for its
/// dep-info record. The record must be rejected even though it carries the same
/// stem, and it is rejected by *name* rather than by a `file_type` check: both
/// are regular files, and the name is what differs. Requiring the `-` separator
/// keeps a stem from matching a longer sibling that merely begins with the same
/// characters.
///
/// The record is excluded by its extension as `Path` reports it, compared
/// case-insensitively: Cargo writes `.d`, but the filesystems it targets do not
/// agree on case, so a case-sensitive suffix test would be the wrong question.
fn witnesses_binary(name: &str, stem: &str) -> bool {
    let Some(rest) = name.strip_prefix(stem) else {
        return false;
    };
    if !rest.starts_with('-') {
        return false;
    }
    !std::path::Path::new(name)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("d"))
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
