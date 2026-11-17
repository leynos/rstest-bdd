//! Identifying the toolchain a preparation stamp is valid against.
//!
//! The fingerprint records this identity so a changed compiler, Cargo or host
//! invalidates every stamp. Both probes need a subprocess, and they live in
//! their own module for exactly that reason: the query that composes the
//! fingerprint stays free of process execution, while the two probes that must
//! run are visible as a single boundary here.
//!
//! Cargo's own identity is not enough. Cargo selects the compiler
//! independently — `RUSTC`, a `build.rustc` configuration, and a `rustup`
//! toolchain all choose it — and fingerprints that compiler separately, so a
//! compiler that changes at a fixed path leaves `cargo -vV` reporting the same
//! text. A stamp written before such a change would then admit a timed
//! assertion whose first nested build recompiled the dependency graph inside
//! its own deadline, which is the failure this module exists to prevent. The
//! compiler is therefore probed in its own right.

use std::{
    env,
    ffi::OsString,
    io,
    path::{Path, PathBuf},
    process::Command,
};

use super::super::NestedCargoEnv;

/// The Cargo probe's label.
const CARGO: &str = "cargo";
/// The compiler probe's label, and the variable that selects it.
const RUSTC: &str = "rustc";

/// Identify the toolchain that produces the prepared artefacts.
///
/// Both halves are probed with `-vV` under `environment`, the filtered
/// environment the nested builds themselves receive, so neither Cargo's
/// selection nor a toolchain override is read from the parent's environment
/// instead of the child's.
///
/// The two identities occupy separate lines, and the fingerprint escapes the
/// joined text onto one line, so a third value cannot be forged from inside
/// either identity. Joining them with a space would let a pair and a single
/// value compose the same text, which is the ambiguity the separation avoids.
///
/// # Errors
///
/// Returns an error when either probe cannot be run, exits non-zero, or prints
/// none of the three identity fields.
pub(super) fn toolchain_identity(environment: &NestedCargoEnv) -> io::Result<String> {
    let cargo = probe(CARGO, Path::new(env!("CARGO")), environment)?;
    let compiler = probe(RUSTC, &compiler_program(environment), environment)?;
    Ok(compose_identity(&cargo, &compiler))
}

/// Join the two probed identities into the text the fingerprint records.
///
/// Separated from the probes so the shape can be asserted without spawning
/// anything, and so each value goes through one escape rule rather than two
/// spellings of it.
fn compose_identity(cargo: &str, compiler: &str) -> String {
    format!("{CARGO}: {cargo}\n{RUSTC}: {compiler}")
}

/// The compiler the nested build will invoke.
///
/// `RUSTC` wins when the child environment sets it, and that is the case the
/// second probe exists for: a path recorded in the fingerprint whose binary is
/// replaced in place is invisible to every other field.
///
/// Otherwise the compiler is the one shipped beside `env!("CARGO")`, which is
/// the Cargo every nested command runs. Resolving it as a path rather than
/// through the child's `PATH` matters because the child's `PATH` is part of
/// what the fingerprint filters, so a fixture's synthetic value would decide
/// which compiler was identified — and a test that never meant to probe a
/// compiler at all would fail for want of one on `PATH`.
///
/// A Cargo with no sibling compiler falls back to the bare program name, the
/// conventional packaging for a separately installed compiler. That name is
/// searched in the environment the command inherits, so it is the last resort
/// rather than the rule.
fn compiler_program(environment: &NestedCargoEnv) -> PathBuf {
    if let Some(configured) = environment.get(RUSTC) {
        return PathBuf::from(configured);
    }
    let name = compiler_name();
    let beside_cargo = Path::new(env!("CARGO")).with_file_name(&name);
    if beside_cargo.is_file() {
        beside_cargo
    } else {
        PathBuf::from(name)
    }
}

/// The compiler's file name, carrying Cargo's own extension.
///
/// A Rust installation that ships `cargo.exe` ships `rustc.exe` beside it, so
/// the sibling lookup has to apply the same extension. A bare `rustc` would
/// never be a file on Windows, and every probe there would take the last-resort
/// branch.
fn compiler_name() -> OsString {
    let cargo = Path::new(env!("CARGO"));
    let mut name = OsString::from(RUSTC);
    if let Some(extension) = cargo.extension() {
        name.push(".");
        name.push(extension);
    }
    name
}

/// Run one `-vV` probe under `environment` and reduce it to its identity.
///
/// The build-relevant variables are overlaid onto the inherited environment
/// rather than replacing it. Overlaying is what makes the probe identify the
/// *child's* compiler: `RUSTC` and `RUSTUP_TOOLCHAIN` from `environment` win,
/// which is the whole question being asked. Clearing instead would also drop
/// `HOME`/`USERPROFILE`, which a `rustup` shim reads to find its toolchains, so
/// a host that installs through `rustup` would fail to identify a compiler the
/// nested build runs perfectly well.
///
/// The program is given as a path wherever it can be, so neither the parent's
/// nor the child's filtered `PATH` has to find it.
fn probe(
    program: &'static str,
    executable: &Path,
    environment: &NestedCargoEnv,
) -> io::Result<String> {
    let mut command = Command::new(executable);
    command.arg("-vV");
    for (key, value) in environment.iter() {
        command.env(key, value);
    }
    let output = command.output().map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "cannot run `{program} -vV` at {}: {error}",
                executable.display()
            ),
        )
    })?;
    identity_from(
        program,
        output.status.success(),
        &String::from_utf8_lossy(&output.stdout),
    )
}

/// The identity text from a completed `-vV` invocation.
///
/// Split from the spawn so the three fields, the non-zero exit and the usable
/// output are all reachable from a test: `-vV` succeeds on every host that can
/// build this crate, so a test that only called the probe could never show that
/// a *failure* is reported rather than replaced by a placeholder. Taking the
/// exit flag rather than a `process::ExitStatus` keeps the fixture portable — a
/// synthetic non-zero status cannot be built on Windows.
///
/// # Errors
///
/// Returns an error when `success` is false or `text` names none of the three
/// fields.
pub(super) fn identity_from(program: &str, success: bool, text: &str) -> io::Result<String> {
    if !success {
        return Err(io::Error::other(format!(
            "`{program} -vV` failed; the toolchain cannot be identified, so no preparation stamp \
             can be trusted against it"
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
        return Err(io::Error::other(format!(
            "`{program} -vV` printed no release, commit-hash or host line; the toolchain cannot \
             be identified, so no preparation stamp can be trusted against it"
        )));
    }
    Ok(kept.join(" "))
}

#[cfg(test)]
mod tests;
