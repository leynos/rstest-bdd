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
        match classify(&entry) {
            Entry::Binary(name) => {
                note_binary(&name, &mut present);
                if present.iter().all(|found| *found) {
                    return Ok(true);
                }
            }
            Entry::Directory(children) => pending.extend(children),
            // A path that vanished, or a pipe or socket, answers nothing
            // either way: the profile is the thing under test, not the race.
            Entry::Other => {}
        }
    }
    Ok(present.iter().all(|found| *found))
}

/// What one directory entry contributes to the walk.
enum Entry {
    /// A file, under the name recorded in its directory entry.
    Binary(std::borrow::Cow<'static, str>),
    /// A directory, with the entries below it to walk next.
    Directory(Vec<std::fs::DirEntry>),
    /// Neither, or a path that vanished between the listing and this read.
    Other,
}

/// Classify `entry` by one `fs::metadata` call.
///
/// `fs::metadata` follows a symlink, so a linked binary still counts: what
/// makes an entry a candidate is being a file at that path, not how the
/// directory entry was recorded. A directory named like the fixture's binary
/// is therefore not a binary, which is the point — its name is the only thing
/// it shares with one.
///
/// The metadata decides both questions at once: a file is a candidate, a
/// directory is descended into, and nothing else is either. Asking the
/// directory first instead costs a failing `ENOTDIR` syscall on every file, in
/// every timed test that calls this.
fn classify(entry: &std::fs::DirEntry) -> Entry {
    match std::fs::metadata(entry.path()) {
        Ok(metadata) if metadata.is_file() => {
            Entry::Binary(entry.file_name().to_string_lossy().into_owned().into())
        }
        Ok(metadata) if metadata.is_dir() => std::fs::read_dir(entry.path())
            .map_or(Entry::Other, |children| {
                Entry::Directory(children.filter_map(std::io::Result::ok).collect())
            }),
        _ => Entry::Other,
    }
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
#[path = "witness/tests.rs"]
mod tests;
