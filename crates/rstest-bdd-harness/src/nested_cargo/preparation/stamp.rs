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
//!
//! # Modules
//!
//! The responsibilities are split so each can be exercised on its own:
//! [`format`] reads and writes the file's grammar, [`fingerprint`] composes the
//! text a stamp is compared against, [`file`] names and reads the file and
//! nothing else, and [`diagnostics`] builds the messages a refusal produces
//! from facts its caller gathered. Their items are re-exported below, so
//! callers keep naming `stamp::` and no call site had to move.
//!
//! Writing is not part of any of them: [`render`] and [`write_stamp`] live in
//! the sibling [`stamp_file`] module, which this one re-exports for the same
//! reason.
//!
//! [`stamp_file`]: super::stamp_file

mod diagnostics;
mod file;
mod fingerprint;
mod format;

pub(in crate::nested_cargo::preparation) use diagnostics::{
    missing_stamp_message,
    unreadable_stamp_error,
};
pub(in crate::nested_cargo::preparation) use file::{matching_stamp, stamp_path};
// The rest of the surface is reached only from a `#[cfg(test)]` module, so each
// is behind the same gate as its user: a release build would otherwise carry
// names nothing reaches. `compose` takes an already-resolved toolchain, which is
// what lets a case hold the compiler fixed while it varies something else.
#[cfg(test)]
pub(in crate::nested_cargo::preparation) use fingerprint::{
    FINGERPRINT_EXACT,
    FINGERPRINT_PREFIXES,
    compose,
    escape,
    is_fingerprint_variable,
};
pub(in crate::nested_cargo::preparation) use fingerprint::{Fingerprint, fingerprint_with};
#[cfg(test)]
pub(in crate::nested_cargo::preparation) use format::parse;
pub(in crate::nested_cargo::preparation) use format::{
    OUTPUTS_SEPARATOR,
    PREPARED_FILE_SUFFIX,
    STAMP_VERSION,
    Stamp,
    StampText,
};

pub(in crate::nested_cargo::preparation) use super::stamp_file::{render, write_stamp};

#[cfg(test)]
mod tests;
