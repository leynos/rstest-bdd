//! Writing a stamp, and the helpers that turn a fingerprint into a file.
//!
//! Separate from `stamp` so neither module has to hold both the wire format
//! and the composition of the text it carries. The two are independently
//! testable: `stamp` composes a fingerprint from a preparation, and this
//! module publishes one atomically and reads it back.

use std::{fmt::Write as _, fs, io, path::Path};

use super::stamp::{Fingerprint, OUTPUTS_SEPARATOR, StampText};

/// Render a stamp file from its fingerprint and recorded artefact names.
pub(super) fn render(fingerprint: &Fingerprint, recorded: &[Vec<String>]) -> StampText {
    // The fingerprint already ends its last line with `\n`, so it is written
    // verbatim: [`parse`] removes exactly that one character to recover the
    // text [`compose`] produced. Normalizing it here instead would silently
    // discard a trailing space in the final field.
    let mut text = String::from(fingerprint.as_str());
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(OUTPUTS_SEPARATOR);
    text.push('\n');
    for section in recorded {
        text.push_str("dir\n");
        for name in section {
            let _ = writeln!(text, "out\t{name}");
        }
    }
    StampText::new(text)
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
