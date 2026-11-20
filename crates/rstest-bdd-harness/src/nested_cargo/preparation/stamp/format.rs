//! The stamp file's format: what a stamp records and how it is read back.
//!
//! A stamp is the fingerprint followed by a separator line and one section per
//! artefact directory. Reading it back means consuming the expected
//! fingerprint first, then the delimiter, then the sections — see [`parse`] for
//! why that order is the only safe one.

use super::fingerprint::Fingerprint;

/// Fingerprint format version; bump to invalidate every existing stamp.
///
/// Version 3 escapes each recorded value onto a single line. Version 2 wrote
/// values verbatim, so a value containing a line break could forge the field
/// that followed it.
///
/// Version 4 records the compiler as well as Cargo. A version-3 stamp carries
/// only Cargo's identity, so without the bump a stamp written before the
/// compiler probe existed would still compose the text this version expects for
/// a stamp that had never recorded a compiler — and the compiler-change
/// invalidation it exists for would not happen on the upgrade itself.
pub(in crate::nested_cargo::preparation) const STAMP_VERSION: &str = "4";

/// Suffix of the hidden stamp file written inside a prepared artefact
/// directory.
pub(in crate::nested_cargo::preparation) const PREPARED_FILE_SUFFIX: &str = ".rstest-bdd-prepared";

/// Line separating the fingerprint from the recorded artefact names.
///
/// One file rather than two keeps the stamp atomic: a single rename publishes
/// the fingerprint and the names it vouches for together, so a reader can
/// never see a new fingerprint beside a stale name list.
pub(in crate::nested_cargo::preparation) const OUTPUTS_SEPARATOR: &str = "@@ recorded-artefacts";

/// A complete stamp file: fingerprint, separator, and recorded names.
///
/// Distinct from [`Fingerprint`] because the two are never interchangeable:
/// a fingerprint is what a stamp is compared *against*, and this is what is
/// written *into* it. Passing the same `String` for both would compile and be
/// wrong.
#[derive(Debug, Clone)]
pub(in crate::nested_cargo::preparation) struct StampText(String);

impl StampText {
    /// Wrap rendered text for writing.
    ///
    /// Only [`render`](super::super::stamp_file::render) calls this, and the field
    /// stays private so that remains true: a `StampText` built anywhere else
    /// could pair a fingerprint with names that preparation never recorded.
    pub(in crate::nested_cargo::preparation) fn new(text: String) -> Self { Self(text) }

    /// The bytes to write to the stamp file.
    pub(in crate::nested_cargo::preparation) fn as_str(&self) -> &str { &self.0 }
}

/// What a matching stamp vouches for.
///
/// Only the recorded names are kept: a `Stamp` exists at all because its
/// fingerprint already matched, so re-storing the text would let a caller
/// compare it again and reach a different answer.
#[derive(Debug)]
pub(in crate::nested_cargo::preparation) struct Stamp {
    /// Artefact names recorded per artefact directory, in
    /// [`Preparation::artefacts`](super::super::Preparation) order.
    pub(in crate::nested_cargo::preparation) recorded: Vec<Vec<String>>,
}

/// Split a stamp file into its fingerprint and recorded names.
///
/// A file with no separator line is a version-1 stamp. Its fingerprint also
/// differs, because [`STAMP_VERSION`] changed, so returning `None` here is a
/// second, independent guard rather than the only one.
pub(in crate::nested_cargo::preparation) fn parse(
    text: &str,
    fingerprint: &Fingerprint,
) -> Option<Stamp> {
    // Consume the expected fingerprint first, then the delimiter, rather than
    // splitting on the delimiter and comparing what came before it. `inputs`
    // is caller-supplied text that `render` writes verbatim ahead of the
    // separator line, so the separator's own spelling is reachable from a
    // value: splitting first found the text inside `inputs: @@ recorded-
    // artefacts` and refused the stamp in the same breath as it was written,
    // turning every gate run into a fresh preparation. Consuming the
    // fingerprint first leaves only the delimiter to find. That ordering is
    // also why this cannot accept a stamp written for different inputs at all:
    // a mismatch fails at the first `strip_prefix`.
    //
    // `render` writes `<fingerprint>\n<separator>\n<sections>` and supplies
    // that first newline whether or not the fingerprint already ends with one,
    // so this must *require* it. Treating it as optional is what let a stamp
    // written for `inputs: h@@ recorded-artefacts` match a fingerprint for
    // `inputs: h`: the parse consumed `h`, still inside the value, then the
    // separator text that the value itself contained, and reported a match.
    // The recorded artefacts of the longer build are ordinarily still on disk,
    // so the gate accepted a preparation whose inputs had genuinely changed.
    //
    // The one whitespace the two are allowed to lose is the line break
    // terminating the fingerprint, which `strip_line_break` removes. Trimming
    // all trailing whitespace instead would make `inputs: h` and `inputs: h `
    // compare equal, the same stale acceptance one character over.
    let rest = text.strip_prefix(strip_line_break(fingerprint.as_str()))?;
    let rest = rest.strip_prefix('\n')?;
    let tail = rest.strip_prefix(OUTPUTS_SEPARATOR)?;
    Some(Stamp {
        recorded: recorded_sections(tail),
    })
}

/// Remove the single line break that terminates a fingerprint.
///
/// [`render`](super::super::stamp_file::render) writes the fingerprint followed by one
/// `\n` and then the separator, so reading a stamp back has to remove exactly
/// that character. The value fields themselves are already line-safe —
/// [`escape`](super::fingerprint::escape) guarantees it — so the only whitespace here that is
/// not significant is that one terminator.
///
/// Using `trim_end` instead would additionally discard trailing spaces and
/// tabs belonging to the last field, so `inputs: h` and `inputs: h ` would
/// match a stamp written for the other. That is a stamp outliving a change to
/// the fixture, which is the invalidation the fingerprint exists to force.
pub(super) fn strip_line_break(text: &str) -> &str { text.strip_suffix('\n').unwrap_or(text) }

/// Read the artefact sections that follow the separator.
///
/// One `dir` line opens a section and each `out` line names an entry in it.
/// Sections arrive in [`Preparation::artefacts`](super::super::Preparation) order,
/// which is why no path is stored: the position is the identity, and a stamp
/// whose section count disagrees with the current description is refused by the
/// caller.
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
