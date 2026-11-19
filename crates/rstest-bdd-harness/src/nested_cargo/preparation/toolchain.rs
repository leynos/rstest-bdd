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
    ffi::{OsStr, OsString},
    io,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use super::{super::NestedCargoEnv, deadline::Deadline};

/// The Cargo probe's label.
const CARGO: &str = "cargo";
/// The compiler probe's label, and the compiler's file name.
const RUSTC: &str = "rustc";
/// The variable that selects the compiler the nested build invokes.
///
/// Kept apart from [`RUSTC`] because the two spell the name differently and
/// [`NestedCargoEnv::get`] matches an environment key exactly: a lookup by the
/// lowercase label never sees `RUSTC=/path/to/compiler`, so the override branch
/// it guards would be unreachable. The label stays lowercase for the probe's
/// messages and the sibling file name, which are case-sensitive on Unix.
const RUSTC_KEY: &str = "RUSTC";

/// Wall-clock bound on one `-vV` probe.
///
/// Sized for a probe rather than a build: an honest `-vV` is milliseconds, and
/// the point is to turn a stalled shim into a named failure. The nested build
/// bound is 300 s because a build has real work to do; a probe that needs even
/// a sixtieth of that is not a probe that is going to finish.
const PROBE_TIMEOUT: Duration = Duration::from_secs(60);

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
pub(super) fn toolchain_identity(
    environment: &NestedCargoEnv,
    working_directory: &Path,
) -> io::Result<String> {
    toolchain_identity_within(environment, working_directory, None)
}

/// [`toolchain_identity`] charged against a shared [`Deadline`].
///
/// The deadline is only meaningful to a caller that also pays for other
/// subprocesses in the same budget — the timed CLI test's closure query, for
/// instance. A caller with nothing else to run passes `None` and gets the
/// per-probe limit alone, which is what every other family wants: their gate
/// has no period of its own to overrun.
///
/// Each probe is bounded by whichever is sooner, the deadline's remainder or
/// [`PROBE_TIMEOUT`], and a deadline already spent returns before the second
/// probe is spawned. That ordering is the point of the type: the compiler probe
/// is the one a spent budget used to reach.
///
/// # Errors
///
/// As [`toolchain_identity`], plus [`io::ErrorKind::TimedOut`] when the
/// deadline is spent before a probe can start.
pub(super) fn toolchain_identity_within(
    environment: &NestedCargoEnv,
    working_directory: &Path,
    deadline: Option<Deadline>,
) -> io::Result<String> {
    let scene = Scene {
        environment,
        working_directory,
    };
    let cargo = probe(CARGO, Path::new(env!("CARGO")), &scene, deadline)?;
    let compiler = probe(
        RUSTC,
        &compiler_program(environment, working_directory),
        &scene,
        deadline,
    )?;
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
///
/// A relative `RUSTC` override is resolved against `working_directory`, the
/// directory the nested build runs in, because that is how Cargo resolves it.
/// [`probe`] spawns the compiler from that same directory, so the path handed
/// back here and the process that resolves it agree: a compiler that exists
/// only beneath the fixture is found, and a `rustup` shim picks the toolchain
/// its directory's `rust-toolchain.toml` names rather than this crate's. An
/// absolute override and a bare name are used unchanged: the first needs no
/// resolution, and the second is looked up through the child's `PATH`.
fn compiler_program(environment: &NestedCargoEnv, working_directory: &Path) -> PathBuf {
    if let Some(configured) = environment.get(RUSTC_KEY) {
        return resolve_compiler_path(configured, working_directory);
    }
    let name = compiler_name();
    let beside_cargo = Path::new(env!("CARGO")).with_file_name(&name);
    if beside_cargo.is_file() {
        beside_cargo
    } else {
        PathBuf::from(name)
    }
}

/// Resolve a `RUSTC` override the way Cargo does.
///
/// An absolute path is returned unchanged. A relative one that names an
/// existing file beneath `working_directory` is joined to it, matching the
/// directory Cargo runs the nested build in. A bare program name — one with no
/// path separator, which Cargo also searches through `PATH` — is left alone so
/// that lookup still happens in the environment the probe inherits rather than
/// being turned into a path that does not exist.
fn resolve_compiler_path(configured: &OsStr, working_directory: &Path) -> PathBuf {
    let path = Path::new(configured);
    if path.is_absolute() || !names_a_location(path) {
        return path.to_path_buf();
    }
    working_directory.join(path)
}

/// Whether `path` names a location rather than a bare program name.
///
/// Cargo treats a `RUSTC` value with no path separator as a name to search for
/// on `PATH`. `Path::parent` is empty for such a name and non-empty for
/// `./tools/rustc` or `tools/rustc`, so it is the check that distinguishes the
/// two without looking at the platform's separator.
fn names_a_location(path: &Path) -> bool {
    path.parent()
        .is_some_and(|parent| !parent.as_os_str().is_empty())
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

/// The conditions a `-vV` probe runs under.
///
/// Bundled because they are never chosen separately: the probe must see the
/// filtered environment the nested build receives *and* start in the directory
/// that build runs in, or it answers about a different toolchain than the one
/// its identity is recorded against. Passing them as one value keeps the two
/// halves from drifting apart at a call site that supplied only one.
struct Scene<'a> {
    /// The filtered environment the nested build receives.
    environment: &'a NestedCargoEnv,
    /// The directory the nested build runs in.
    working_directory: &'a Path,
}

/// Run one `-vV` probe under `scene`.
///
/// The build-relevant variables are overlaid onto the inherited environment
/// rather than replacing it. Overlaying is what makes the probe identify the
/// *child's* compiler: `RUSTC` and `RUSTUP_TOOLCHAIN` from `environment` win,
/// which is the whole question being asked. Clearing instead would also drop
/// `HOME`/`USERPROFILE`, which a `rustup` shim reads to find its toolchains, so
/// a host that installs through `rustup` would fail to identify a compiler the
/// nested build runs perfectly well.
///
/// The probe runs from `working_directory`, the directory the nested build
/// runs in, for the same reason: Cargo resolves a relative `RUSTC` and a
/// relative `CARGO_HOME` against the command's directory, and a plain
/// `rustc` on `PATH` is usually a `rustup` shim that selects its toolchain
/// from the `rust-toolchain.toml` it finds by walking up from there. A probe
/// left in this process's directory would answer for a different toolchain
/// than the build it is supposed to describe — and a shim with no toolchain
/// file above the test binary's directory fails outright, taking every
/// preparation with it.
///
/// The program is given as a path wherever it can be, so neither the parent's
/// nor the child's filtered `PATH` has to find it.
fn probe(
    program: &'static str,
    executable: &Path,
    scene: &Scene<'_>,
    deadline: Option<Deadline>,
) -> io::Result<String> {
    // Resolved before the command is built, so a spent budget reports the
    // deadline rather than spawning a process with no time to run.
    let bound = probe_bound(deadline, program)?;
    let mut command = Command::new(executable);
    command.arg("-vV");
    // A working directory that has not been created yet is not a reason to
    // refuse the identity: the preparation creates the tree it stamps, so on
    // the very first run the directory can be absent. Falling back to the
    // inherited directory keeps the probe runnable, and the nested build that
    // follows creates the tree and runs there.
    if scene.working_directory.is_dir() {
        command.current_dir(scene.working_directory);
    }
    for (key, value) in scene.environment.iter() {
        command.env(key, value);
    }
    // Bounded rather than `output()`, because these probes run inside every
    // timed test body: `lifecycle` asks for the identity on each gate. A
    // `rustup` shim that resolves its toolchain over the network, or a
    // compiler on a stalled mount, would otherwise hang the test and let the
    // runner report its own timeout instead of naming the probe. `-vV` is a
    // few milliseconds' work on any healthy host, so the bound is generous
    // against real work and still far below the runner's.
    let output = super::super::run_bounded_with_timeout(&mut command, bound).map_err(|error| {
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

/// The wall-clock bound for one `-vV` probe.
///
/// Split from [`probe`] so the rule the shared deadline exists for is
/// assertable without spawning anything: `None` is the per-probe limit alone,
/// and `Some(deadline)` is the smaller of that limit and what the budget has
/// left, refusing outright when nothing remains. A caller that asked the
/// budget first and then substituted the per-probe limit would reintroduce the
/// slow-success failure this split exists to prevent.
///
/// # Errors
///
/// As [`Deadline::cap`]: [`io::ErrorKind::TimedOut`] naming the probe when the
/// budget is spent.
pub(super) fn probe_bound(deadline: Option<Deadline>, program: &str) -> io::Result<Duration> {
    deadline.map_or(Ok(PROBE_TIMEOUT), |deadline| {
        deadline.cap(PROBE_TIMEOUT, &format!("`{program} -vV`"))
    })
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
