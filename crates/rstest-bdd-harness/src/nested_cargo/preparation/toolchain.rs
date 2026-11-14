//! Identifying the toolchain a preparation stamp is valid against.
//!
//! The fingerprint records this identity so a changed compiler, Cargo or host
//! invalidates every stamp. It is the only part of the fingerprint that needs a
//! subprocess, and it is kept in its own module for exactly that reason: the
//! query that composes the fingerprint stays free of process execution, while
//! the one probe that must run is visible as a single boundary here.

use std::{env, io, process::Command};

/// Identify the toolchain that produced the prepared artefacts.
///
/// `cargo -vV` reports the release, commit and host of the Cargo in use, which
/// changes whenever the toolchain does. Resolved from the `CARGO` compile-time
/// variable, never a `PATH` lookup.
///
/// # Errors
///
/// Returns an error when `cargo -vV` cannot be run, exits non-zero, or prints
/// none of the three fields.
pub(super) fn toolchain_identity() -> io::Result<String> {
    let output = Command::new(env!("CARGO")).arg("-vV").output()?;
    identity_from(
        output.status.success(),
        &String::from_utf8_lossy(&output.stdout),
    )
}

/// The identity text from a completed `cargo -vV` invocation.
///
/// Split from the spawn so the three fields, the non-zero exit and the usable
/// output are all reachable from a test: `cargo -vV` itself succeeds on every
/// host that can build this crate, so a test that only called it could never
/// show that a *failure* is reported rather than replaced by a placeholder.
/// Taking the exit flag rather than a `process::ExitStatus` keeps the fixture
/// portable — a synthetic non-zero status cannot be built on Windows.
///
/// # Errors
///
/// Returns an error when `success` is false or `text` names none of the three
/// fields.
pub(super) fn identity_from(success: bool, text: &str) -> io::Result<String> {
    if !success {
        return Err(io::Error::other(concat!(
            "`cargo -vV` failed; the toolchain cannot be identified, so no ",
            "preparation stamp can be trusted against it"
        )));
    }
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

#[cfg(test)]
mod tests;
