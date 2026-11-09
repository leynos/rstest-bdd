//! The stamp protocol that records which fixture artefacts are prepared.
//!
//! A stamp is a small hidden file inside the artefact directory it protects.
//! It records a fingerprint of everything that changes the compiled output —
//! the fingerprint format version, the toolchain identity, the build-relevant
//! environment, a caller-supplied variant label and a caller-supplied input
//! hash — so a changed compiler, feature set or fixture source forces
//! preparation again. Because the stamp lives inside the directory it
//! describes, evicting that directory also evicts the stamp.
//!
//! A fingerprint answers "the inputs have not changed". It cannot answer "the
//! compiled units are still there": the profile directory holding them need
//! not be inside the family's stamp root. So the stamp also records the
//! artefact names the preparation left behind, and the gate requires every one
//! of them to still exist. Recording what was actually produced is the only
//! exact answer available: no rule about rustc's file naming can tell a
//! metadata-only unit, which Cargo reuses, from a linked unit whose `.rlib`
//! was removed, which Cargo rebuilds.
//!
//! The stamp is written last, after every preparation command succeeded, and
//! replaced atomically through a temporary sibling, so a killed preparation
//! never looks complete.

use std::{
    env,
    fmt::Write as _,
    fs,
    io,
    path::{Path, PathBuf},
    process::Command,
};

use super::Preparation;

/// Fingerprint format version; bump to invalidate every existing stamp.
pub(super) const STAMP_VERSION: &str = "2";

/// Suffix of the hidden stamp file written inside a prepared artefact
/// directory.
pub(super) const PREPARED_FILE_SUFFIX: &str = ".rstest-bdd-prepared";

/// Line separating the fingerprint from the recorded artefact names.
///
/// One file rather than two keeps the stamp atomic: a single rename publishes
/// the fingerprint and the names it vouches for together, so a reader can
/// never see a new fingerprint beside a stale name list.
const OUTPUTS_SEPARATOR: &str = "@@ recorded-artefacts";

/// Environment variables that change what a nested preparation builds.
///
/// Everything else in the child environment (`NEXTEST_*`,
/// `LLVM_PROFILE_FILE`, thread counts, logging filters) is excluded: a change
/// to those must not force a rebuild.
pub(super) const FINGERPRINT_EXACT: [&str; 5] = [
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_INCREMENTAL",
    "RUSTDOCFLAGS",
    "RUSTUP_TOOLCHAIN",
];

/// Prefixes whose whole variable set changes the compiled artefacts.
pub(super) const FINGERPRINT_PREFIXES: [&str; 5] = [
    "RUSTC",
    "RUSTDOC",
    "CARGO_BUILD_",
    "CARGO_PROFILE_",
    "CARGO_TARGET_",
];

/// The text a stamp was written from.
///
/// A newtype rather than a bare `String` because this text is compared for
/// equality, is written into a file, and decides whether a timed assertion may
/// run at all. A bare string at every one of those points invites passing the
/// stamp's own contents where the freshly computed text belongs, and the two
/// are different strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Fingerprint(String);

impl Fingerprint {
    /// The text recorded in a stamp file.
    pub(super) fn as_str(&self) -> &str { &self.0 }
}

/// A complete stamp file: fingerprint, separator, and recorded names.
///
/// Distinct from [`Fingerprint`] because the two are never interchangeable:
/// a fingerprint is what a stamp is compared *against*, and this is what is
/// written *into* it. Passing the same `String` for both would compile and be
/// wrong.
#[derive(Debug, Clone)]
pub(super) struct StampText(String);

impl StampText {
    /// The bytes to write to the stamp file.
    pub(super) fn as_str(&self) -> &str { &self.0 }
}

/// What a matching stamp vouches for.
///
/// Only the recorded names are kept: a `Stamp` exists at all because its
/// fingerprint already matched, so re-storing the text would let a caller
/// compare it again and reach a different answer.
pub(super) struct Stamp {
    /// Artefact names recorded per artefact directory, in
    /// [`Preparation::artefacts`] order.
    pub(super) recorded: Vec<Vec<String>>,
}

/// The stamp file protecting `preparation.directory`.
pub(super) fn stamp_path(preparation: &Preparation) -> PathBuf {
    preparation
        .directory
        .join(format!(".{}{}", preparation.family, PREPARED_FILE_SUFFIX))
}

/// The stamp at `stamp`, if it was written for exactly `fingerprint`.
///
/// Returns `Ok(None)` for a missing stamp and for one whose fingerprint
/// differs. Both are "not prepared", which is the safe answer: the caller
/// prepares again or fails fast rather than admitting a timed assertion
/// against a tree the stamp does not describe.
///
/// # Errors
///
/// Returns the read error when the stamp exists but cannot be read. Treating
/// that as "not prepared" would silently discard a stamp that may well
/// describe a warm tree, and the caller would pay for the rebuild with no
/// indication of why. Reporting it keeps the cause visible; the caller still
/// re-prepares.
pub(super) fn matching_stamp(stamp: &Path, fingerprint: &Fingerprint) -> io::Result<Option<Stamp>> {
    let text = match fs::read_to_string(stamp) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    Ok(parse(&text, fingerprint))
}

/// The matching stamp, reporting an unreadable one before re-preparing.
///
/// An unreadable stamp is neither a match nor an absence. Every caller has to
/// re-prepare either way, so the read failure is not fatal; it is reported so a
/// tree that was warm for reasons the reader cannot see does not turn into an
/// unexplained rebuild.
pub(super) fn matching_stamp_or_note(stamp: &Path, fingerprint: &Fingerprint) -> Option<Stamp> {
    match matching_stamp(stamp, fingerprint) {
        Ok(matching) => matching,
        Err(error) => {
            note(&format!(
                "could not read stamp {}: {error}; preparing again",
                stamp.display()
            ));
            None
        }
    }
}

/// Split a stamp file into its fingerprint and recorded names.
///
/// A file with no separator line is a version-1 stamp. Its fingerprint also
/// differs, because [`STAMP_VERSION`] changed, so returning `None` here is a
/// second, independent guard rather than the only one.
fn parse(text: &str, fingerprint: &Fingerprint) -> Option<Stamp> {
    let (head, tail) = text.split_once(OUTPUTS_SEPARATOR)?;
    if head.trim_end() != fingerprint.as_str().trim_end() {
        return None;
    }
    Some(Stamp {
        recorded: recorded_sections(tail),
    })
}

/// Read the artefact sections that follow the separator.
///
/// One `dir` line opens a section and each `out` line names an entry in it.
/// Sections arrive in [`Preparation::artefacts`] order, which is why no path
/// is stored: the position is the identity, and a stamp whose section count
/// disagrees with the current description is refused by the caller.
fn recorded_sections(tail: &str) -> Vec<Vec<String>> {
    let mut sections: Vec<Vec<String>> = Vec::new();
    for line in tail.lines() {
        if line == "dir" {
            sections.push(Vec::new());
        } else if let Some(name) = line.strip_prefix("out\t")
            && let Some(section) = sections.last_mut()
        {
            section.push(name.to_owned());
        }
    }
    sections
}

/// Render a stamp file from its fingerprint and recorded artefact names.
pub(super) fn render(fingerprint: &Fingerprint, recorded: &[Vec<String>]) -> StampText {
    let mut text = String::from(fingerprint.as_str().trim_end());
    text.push('\n');
    text.push_str(OUTPUTS_SEPARATOR);
    text.push('\n');
    for section in recorded {
        text.push_str("dir\n");
        for name in section {
            let _ = writeln!(text, "out\t{name}");
        }
    }
    StampText(text)
}

/// Write the stamp atomically, so a killed preparation never looks complete.
pub(super) fn write_stamp(stamp: &Path, stamp_text: &StampText) -> io::Result<()> {
    let Some(parent) = stamp.parent() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("stamp path {} has no parent directory", stamp.display()),
        ));
    };
    fs::create_dir_all(parent)?;
    let stamp_name = stamp.file_name().map_or_else(
        || String::from("prepared"),
        |name| name.to_string_lossy().into_owned(),
    );
    let temporary = parent.join(format!(".{stamp_name}.{}.tmp", std::process::id()));
    fs::write(&temporary, stamp_text.as_str())?;
    fs::rename(&temporary, stamp)
}

/// The fingerprint text recorded in the stamp.
///
/// # Errors
///
/// Returns an error when the toolchain cannot be identified. A fingerprint
/// that could not name the toolchain would be the *same* text on a host where
/// identification fails as on the run before, so a stamp would survive a
/// toolchain change — exactly the invalidation the field exists to force.
/// Reporting the failure is the only answer that fails closed without
/// re-preparing on every single run.
pub(super) fn fingerprint(preparation: &Preparation) -> io::Result<Fingerprint> {
    fingerprint_from(preparation, toolchain_identity)
}

/// The fingerprint text, with the toolchain identity resolved by `resolve`.
///
/// [`toolchain_identity`] spawns `cargo -vV`, which a unit test cannot make
/// fail on demand, yet its failure is the branch that decides whether a stamp
/// may be trusted at all. Taking the resolver as a parameter keeps that branch
/// reachable from a test while leaving production callers on the real probe.
pub(super) fn fingerprint_from(
    preparation: &Preparation,
    resolve: impl FnOnce() -> io::Result<String>,
) -> io::Result<Fingerprint> {
    Ok(compose(preparation, &resolve()?))
}

/// Compose the fingerprint text from an already-resolved toolchain identity.
///
/// Separated from [`fingerprint`] so the text can be exercised against a chosen
/// identity: [`toolchain_identity`] runs `cargo -vV`, which a unit test cannot
/// vary, yet the identity decides whether a stamp survives a compiler change.
/// Composition is therefore free of process execution, and infallible — the
/// only fallible step left in [`fingerprint`] is resolving the identity.
pub(super) fn compose(preparation: &Preparation, toolchain: &str) -> Fingerprint {
    let mut text = String::new();
    let _ = writeln!(text, "version: {STAMP_VERSION}");
    let _ = writeln!(text, "toolchain: {toolchain}");
    for (key, value) in preparation.environment.iter() {
        let key_text = key.to_string_lossy();
        if is_fingerprint_variable(&key_text) {
            let _ = writeln!(text, "{key_text}={}", value.to_string_lossy());
        }
    }
    let _ = writeln!(text, "variant: {}", preparation.variant);
    let _ = writeln!(text, "inputs: {}", preparation.inputs);
    Fingerprint(text)
}

/// Whether `key` influences the compiled artefacts.
fn is_fingerprint_variable(key: &str) -> bool {
    FINGERPRINT_EXACT.contains(&key)
        || FINGERPRINT_PREFIXES
            .iter()
            .any(|prefix| key.starts_with(prefix))
}

/// The fail-fast message for a timed test with no prepared artefacts.
///
/// # Errors
///
/// Returns the read error when an artefact directory cannot be inspected, so
/// the caller reports the real cause rather than a message assembled from a
/// partial inspection.
pub(super) fn missing_stamp_message(
    preparation: &Preparation,
    stamp: &Path,
    recorded: Option<&Stamp>,
) -> io::Result<String> {
    let mut reason = format!("{} is missing or stale", stamp.display());
    // The same predicate the gate uses, so the message names exactly the
    // directories and artefacts whose absence refused it — including a profile
    // that still exists but has lost `deps`, `.fingerprint` or a recorded unit.
    let evictions = super::artefacts::evicted(preparation, recorded)?;
    let evicted: Vec<String> = evictions
        .iter()
        .map(super::artefacts::Eviction::describe)
        .collect();
    if !evicted.is_empty() {
        let _ = write!(
            reason,
            ", and the artefacts it vouched for are gone ({})",
            evicted.join(", ")
        );
    }
    Ok(format!(
        "nested fixture artefacts for family `{}` are not prepared ({reason}).\nUnder nextest \
         every test runs in its own process, so preparing here would make a timed test pay for \
         the cold build it is meant to avoid.\nPrepare them first with `make \
         prepare-nested-fixtures` (or `cargo nextest run -E \
         'test(/nested_fixture_preparation$/)`), under the same `RUSTFLAGS` and feature selection \
         as the run that reads this stamp: the fingerprint covers both, so preparing under a \
         different environment only warms a tree this run will not reuse.",
        preparation.family
    ))
}

/// Identify the toolchain that produced the prepared artefacts.
///
/// `cargo -vV` reports the release, commit and host of the Cargo in use, which
/// changes whenever the toolchain does. Resolved from the `CARGO`
/// compile-time variable, never a `PATH` lookup.
///
/// # Errors
///
/// Returns an error when `cargo -vV` cannot be run, exits non-zero, or prints
/// none of the three fields. See [`fingerprint`] for why this is an error
/// rather than an `"unknown"` placeholder.
fn toolchain_identity() -> io::Result<String> {
    let output = Command::new(env!("CARGO")).arg("-vV").output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            concat!(
                "`cargo -vV` failed with {}; the toolchain cannot be identified, so no ",
                "preparation stamp can be trusted against it"
            ),
            output.status
        )));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let kept: Vec<&str> = text
        .lines()
        .filter(|line| {
            line.starts_with("release:")
                || line.starts_with("commit-hash:")
                || line.starts_with("host:")
        })
        .collect();
    if kept.is_empty() {
        return Err(io::Error::other(concat!(
            "`cargo -vV` printed no release, commit-hash or host line; the toolchain ",
            "cannot be identified, so no preparation stamp can be trusted against it"
        )));
    }
    Ok(kept.join(" "))
}

/// Report preparation progress on standard error.
///
/// Preparation is intentionally visible in test logs: its elapsed time and
/// compiled-unit count are the evidence that the cold work left the timed
/// assertions.
pub(super) fn note(message: &str) {
    use std::io::Write as _;
    let _ = writeln!(io::stderr(), "nested preparation: {message}");
}

#[cfg(test)]
mod tests;
