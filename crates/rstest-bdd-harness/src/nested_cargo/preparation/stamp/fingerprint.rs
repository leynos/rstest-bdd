//! Composing the fingerprint a stamp is compared against.
//!
//! The text covers everything that changes the compiled output: the format
//! version, the toolchain identity, the build-relevant environment, the
//! caller's variant label and the caller's input hash. Composition itself
//! spawns nothing — the toolchain is resolved at the boundary and passed in —
//! which is what lets the text be exercised against a chosen identity.

use std::fmt::Write as _;

use super::{super::Preparation, format::STAMP_VERSION};

/// Environment variables that change what a nested preparation builds.
///
/// Everything else in the child environment (`NEXTEST_*`,
/// `LLVM_PROFILE_FILE`, thread counts, logging filters) is excluded: a change
/// to those must not force a rebuild.
pub(in crate::nested_cargo::preparation) const FINGERPRINT_EXACT: [&str; 5] = [
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_INCREMENTAL",
    "RUSTDOCFLAGS",
    "RUSTUP_TOOLCHAIN",
];

/// Prefixes whose whole variable set changes the compiled artefacts.
pub(in crate::nested_cargo::preparation) const FINGERPRINT_PREFIXES: [&str; 5] = [
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
///
/// The field is reachable from the parent module and its descendants only,
/// which is what lets the round-trip cases in [`super::tests`] write a
/// fingerprint by hand — the one way to state a text `compose` would never
/// produce. Callers outside the stamp modules reach the text through
/// [`as_str`](Fingerprint::as_str) and build it through
/// [`compose`](super::compose) or [`fingerprint_with`](super::fingerprint_with).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::nested_cargo::preparation) struct Fingerprint(pub(super) String);

impl Fingerprint {
    /// The text recorded in a stamp file.
    pub(in crate::nested_cargo::preparation) fn as_str(&self) -> &str { &self.0 }
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
/// reach [`fingerprint_with`] through `stamp::read_stamp`, which supplies the
/// real probe, so a second entry point that also resolved the toolchain would
/// be a second place for the probe's error to be swallowed.
///
/// # Errors
///
/// Returns the probe's error unchanged. The caller decides what a missing
/// identity means; swallowing it here is the one thing this function must not
/// do, because the moment an unidentifiable toolchain composes the same text
/// as the run before, the stamp outlives the compiler it describes.
pub(in crate::nested_cargo::preparation) fn fingerprint_with<P>(
    preparation: &Preparation,
    probe: P,
) -> std::io::Result<Fingerprint>
where
    P: FnOnce() -> std::io::Result<String>,
{
    let toolchain = probe()?;
    Ok(compose(preparation, &toolchain))
}

/// Compose the fingerprint text from an already-resolved toolchain identity.
///
/// Separated from [`fingerprint_with`] so the text can be exercised against a
/// chosen identity: `toolchain_identity` spawns `cargo -vV` and `rustc -vV`, which a
/// unit test cannot vary, yet the identity decides whether a stamp survives a
/// compiler change. Composition is therefore free of process execution, and
/// infallible.
pub(in crate::nested_cargo::preparation) fn compose(
    preparation: &Preparation,
    toolchain: &str,
) -> Fingerprint {
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
pub(in crate::nested_cargo::preparation) fn escape(value: &str) -> String {
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
pub(in crate::nested_cargo::preparation) fn is_fingerprint_variable(key: &str) -> bool {
    FINGERPRINT_EXACT.contains(&key)
        || FINGERPRINT_PREFIXES
            .iter()
            .any(|prefix| key.starts_with(prefix))
}
