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
    fmt::Write as _,
    fs,
    io,
    path::{Path, PathBuf},
};

use super::Preparation;

/// Fingerprint format version; bump to invalidate every existing stamp.
///
/// Version 3 escapes each recorded value onto a single line. Version 2 wrote
/// values verbatim, so a value containing a line break could forge the field
/// that followed it.
pub(super) const STAMP_VERSION: &str = "3";

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

/// The stamp at `stamp` if it was written for exactly `fingerprint`.
///
/// Returns `Ok(None)` for a missing stamp and for one whose fingerprint
/// differs. Both are "not prepared", which is the safe answer: the caller
/// prepares again or fails fast rather than admitting a timed assertion
/// against a tree the stamp does not describe.
///
/// An unreadable stamp is neither a match nor an absence, and the two runners
/// reach opposite conclusions from it: under `nextest` a non-match fails the
/// test, while under `cargo test` it re-prepares. Deciding that here would
/// force the `nextest` case to report a missing stamp for a tree that is
/// present but unreadable — a diagnosis the reader cannot act on. Returning the
/// error keeps the choice, and the cause, at the boundary that owns the runner
/// policy.
///
/// This query is silent by design. It reports what it found and nothing else,
/// so the boundary that applies the runner policy is also the one place that
/// decides what to log. Logging here as well would write the same failure twice
/// on the `cargo test` path, and would put a side effect on a function whose
/// contract is otherwise just a read.
///
/// # Errors
///
/// Returns the read error when the stamp exists but cannot be read. Treating
/// that as "not prepared" would silently discard a stamp that may well
/// describe a warm tree, and the caller would pay for the rebuild with no
/// indication of why.
pub(super) fn matching_stamp(stamp: &Path, fingerprint: &Fingerprint) -> io::Result<Option<Stamp>> {
    let text = match fs::read_to_string(stamp) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    Ok(parse(&text, fingerprint))
}

/// The fail-fast error for a stamp that exists but cannot be read.
///
/// Under `nextest` this is a distinct condition from a missing stamp: the tree
/// is there, and the reader cannot say whether it matches. Reporting it as
/// "missing or stale" would send the reader looking for an evicted directory
/// that is present, so the message names the read failure and the remedy.
pub(super) fn unreadable_stamp_error(stamp: &Path, error: &io::Error) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "the preparation stamp {} exists but cannot be read ({error}), so the prepared \
             artefacts cannot be vouched for; re-run `make prepare-nested-fixtures`",
            stamp.display()
        ),
    )
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
/// The toolchain is resolved first, at this boundary, and the text is then
/// composed from the resolved identity. Composition itself reads no process
/// state: everything it needs has already been resolved into `preparation` or
/// is passed in as `toolchain`. Keeping the probe here — rather than inside the
/// query — is what lets the composition be exercised against a chosen identity
/// and the probe's failure be exercised independently.
///
/// There is no infallible [`fingerprint`](self) wrapper: production callers
/// reach [`fingerprint_with`] through `lifecycle`, which supplies the real
/// probe, so a second entry point that also resolved the toolchain would be a
/// second place for the probe's error to be swallowed.
///
/// # Errors
///
/// Returns the probe's error unchanged. The caller decides what a missing
/// identity means; swallowing it here is the one thing this function must not
/// do, because the moment an unidentifiable toolchain composes the same text
/// as the run before, the stamp outlives the compiler it describes.
pub(super) fn fingerprint_with<P>(preparation: &Preparation, probe: P) -> io::Result<Fingerprint>
where
    P: FnOnce() -> io::Result<String>,
{
    let toolchain = probe()?;
    Ok(compose(preparation, &toolchain))
}

/// Compose the fingerprint text from an already-resolved toolchain identity.
///
/// Separated from [`fingerprint`] so the text can be exercised against a chosen
/// identity: [`toolchain_identity`] runs `cargo -vV`, which a unit test cannot
/// vary, yet the identity decides whether a stamp survives a compiler change.
/// Composition is therefore free of process execution, and infallible.
pub(super) fn compose(preparation: &Preparation, toolchain: &str) -> Fingerprint {
    let mut text = String::new();
    let _ = writeln!(text, "version: {STAMP_VERSION}");
    let _ = writeln!(text, "toolchain: {}", escape(toolchain));
    for (key, value) in preparation.environment.iter() {
        let key_text = key.to_string_lossy();
        if is_fingerprint_variable(&key_text) {
            let _ = writeln!(text, "{key_text}={}", escape(&value.to_string_lossy()));
        }
    }
    let _ = writeln!(text, "variant: {}", escape(&preparation.variant));
    let _ = writeln!(text, "inputs: {}", escape(&preparation.inputs));
    Fingerprint(text)
}

/// Escape one recorded value so it occupies exactly one line.
///
/// Every field is one `name: value` or `name=value` line, and the fingerprint
/// is compared as text, so a value carrying a line break could otherwise write
/// what looks like the next field: a `RUSTFLAGS` of `"-Dwarnings\ninputs: x"`
/// composes exactly the same text as the same flags with an `inputs` of `x`.
/// Two different builds would then share a stamp, which is the invalidation
/// this fingerprint exists to force. `\` and the two line breaks are the only
/// characters escaped, so ordinary values are recorded unchanged and remain
/// readable in the stamp file.
pub(super) fn escape(value: &str) -> String {
    if !value.contains(['\\', '\n', '\r']) {
        return value.to_owned();
    }
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// Whether `key` influences the compiled artefacts.
pub(super) fn is_fingerprint_variable(key: &str) -> bool {
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
