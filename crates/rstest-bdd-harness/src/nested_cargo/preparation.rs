//! Shared preparation policy for cold nested-Cargo fixture builds.
//!
//! Several integration suites drive real `cargo` invocations against fixture
//! crates. When a fixture has never been built, or its target directory has
//! been evicted, that build compiles a dependency graph from scratch inside
//! the test body, so the test's own deadline pays for work the test does not
//! assert on. `cargo-bdd::cli::list_steps_runs`,
//! `rstest-bdd::trybuild_macros::runner::step_macros_compile` and the
//! `feature_rebuild_invalidation` scenarios each exhausted their configured
//! limits that way.
//!
//! This module moves the cold work into explicitly named preparation tests
//! without widening any assertion deadline:
//!
//! * each preparation family writes a stamp inside the artefact directory it protects, and only
//!   after its last preparation command succeeded, so evicting the target directory also evicts the
//!   stamp;
//! * the stamp records a fingerprint of the build-relevant environment, the toolchain identity, a
//!   caller-supplied variant label and a caller-supplied hash of the fixture inputs, so a changed
//!   compiler, feature set or fixture source forces preparation again;
//! * under `cargo nextest` a missing or stale stamp fails fast with the command that prepares it,
//!   because nextest runs every test in its own process and a timed test cannot prepare without
//!   paying the cost it is meant to avoid;
//! * under plain `cargo test`, preparation runs lazily under a process-wide lock, so the same
//!   suites stay usable without the extra Nextest pass. `#[serial]` serializes within a single
//!   process only; under nextest, ordering across processes comes from the `cargo-spawning` test
//!   group, so a family whose test binary shares a directory must stay in that group.
//!
//! Preparation commands are bounded by [`PREPARATION_COMMAND_TIMEOUT`], which
//! is deliberately separate from every assertion deadline. The stamp protocol
//! lives in the private `stamp` submodule and input hashing in `input_hash`;
//! only [`input_hash`] is re-exported.

mod artefacts;
mod input_hash;
mod stamp;

pub use artefacts::DEPENDENCY_ARTEFACT_DIRS;

#[cfg(test)]
mod tests;

use std::{
    env,
    io,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

pub use input_hash::{cargo_config_inputs, input_hash};
use stamp::{fingerprint, missing_stamp_message, note, stamp_matches, stamp_path, write_stamp};

use super::{NestedCargoEnv, ProfileDestination, env_from_vars};

/// Wall-clock bound for one nested preparation command.
///
/// A preparation command compiles what the timed tests would otherwise
/// compile inside their own deadlines, so its bound is sized for a queued cold
/// build on a two-vCPU CI runner rather than for a warm assertion. It is not,
/// and must not become, the deadline of any test.
///
/// It must stay **below** the 30-minute `slow-timeout` that `.config/nextest.toml`
/// gives `nested_fixture_preparation`, even when a preparation test runs several
/// commands in sequence: a combined bound at or above the test's own budget
/// would let nextest kill the test first, discarding the captured child output
/// that [`run_bounded_with_timeout`] puts in the error. The worst case is
/// [`MAX_SEQUENTIAL_COMMANDS`] commands — the rebuild-invalidation preparation
/// compiles one fixture, then runs a metadata preflight and a second compile —
/// so nine minutes keeps three of them at 27 minutes, under the 30-minute
/// period.
///
/// Nine minutes is still far above any command measured on CI. The whole
/// three-command preparation test took 31.154 s on the cold run
/// `41981885921`, and a command cannot outlast the test that runs it, so every
/// command inside that test is covered many times over.
///
/// [`run_bounded_with_timeout`]: super::run_bounded_with_timeout
pub const PREPARATION_COMMAND_TIMEOUT: Duration = Duration::from_secs(9 * 60);

/// The most bounded commands one preparation test runs in sequence.
///
/// `.config/nextest.toml` gives the preparation tests a single `slow-timeout`
/// period, and the per-command bound above must keep this many commands inside
/// it. The rebuild-invalidation preparation is the worst case: it compiles one
/// fixture, then runs the addition fixture's metadata preflight and its
/// compile. Every other family runs a single bounded command.
///
/// A new family that runs more commands than this must raise it and shrink
/// [`PREPARATION_COMMAND_TIMEOUT`] to match, which
/// `preparation_command_bound_fires_before_the_runner_kills_the_test` enforces
/// rather than trusting this comment.
pub const MAX_SEQUENTIAL_COMMANDS: u64 = 3;

/// Process-wide lock serializing lazy preparation under plain `cargo test`.
static PREPARE_LOCK: Mutex<()> = Mutex::new(());

/// Cargo's development profile directory, relative to a target directory.
///
/// Every nested build here runs without `--release` or `--profile`, so its
/// compiled units land in `<target>/debug`. This is the directory a timed
/// assertion reuses, and it is not always inside the directory the stamp is
/// written to: for the scratch families it is a sibling of the stamp root, and
/// for a family whose stamped directory *is* its child's `CARGO_TARGET_DIR` it
/// is a descendant. Either way it is the directory
/// [`Preparation::artefacts`] names.
pub const DEFAULT_PROFILE: &str = "debug";

/// What a preparation builds, as the caller describes it.
///
/// These three travel together: a family is named, it builds a variant, and
/// what it builds is identified by a hash of its inputs. Grouping them keeps
/// [`Preparation::new`] readable and stops a caller from pairing one family's
/// name with another's input hash.
#[derive(Debug, Clone)]
pub struct Sources {
    /// Stable kebab-case family name, used in the stamp file name and logs.
    pub family: String,
    /// Caller-supplied label for the feature set or tool variant, such as
    /// `all-features` or the resolved Clippy version.
    pub variant: String,
    /// Caller-supplied hash of the fixture sources and lockfiles.
    pub inputs: String,
}

impl Sources {
    /// Name a family, its variant, and the hash of the inputs it builds.
    #[must_use]
    pub fn new(
        family: impl Into<String>,
        variant: impl Into<String>,
        inputs: impl Into<String>,
    ) -> Self {
        Self {
            family: family.into(),
            variant: variant.into(),
            inputs: inputs.into(),
        }
    }
}

/// One family of prepared fixture artefacts.
#[derive(Debug, Clone)]
pub struct Preparation {
    /// Stable kebab-case family name, used in the stamp file name and logs.
    pub family: String,
    /// Directory the prepared artefacts live in, and where the stamp is
    /// written.
    pub directory: PathBuf,
    /// The filtered environment the preparation commands run with.
    pub environment: NestedCargoEnv,
    /// Caller-supplied label for the feature set or tool variant, such as
    /// `all-features` or the resolved Clippy version.
    pub variant: String,
    /// Caller-supplied hash of the fixture sources and lockfiles.
    pub inputs: String,
    /// Directories whose contents the stamp vouches for.
    ///
    /// A stamp lives at [`Preparation::directory`], which for the scratch
    /// families is their own root — `<target>/tests/<family>`. The compiled
    /// units a timed assertion actually reuses are written to Cargo's profile
    /// directory, `<target>/debug`, and for those families that is a *sibling*
    /// of the stamp's directory rather than a descendant of it. Evicting only
    /// `<target>/debug` therefore leaves the stamp intact while removing the
    /// dependency graph, so a stamp alone cannot answer "are the artefacts
    /// still there".
    ///
    /// The gate requires every directory listed here to exist. A missing one
    /// refuses the preparation exactly as a missing stamp does, which is what
    /// keeps a timed assertion from rebuilding the graph inside its deadline.
    pub artefacts: Vec<PathBuf>,
}

impl Preparation {
    /// Build a preparation description.
    ///
    /// The artefact directory is derived from `environment`, not from
    /// `directory`: [`Preparation::directory`] is where the stamp is written,
    /// while the compiled units land in the profile directory of the target
    /// directory the *child* resolves. Deriving both from one value is what
    /// keeps them in step when the target directory moves.
    #[must_use]
    pub fn new(sources: Sources, directory: PathBuf, environment: NestedCargoEnv) -> Self {
        let artefacts = environment
            .target_directory()
            .map(|target| vec![target.join(DEFAULT_PROFILE)])
            .unwrap_or_default();
        Self {
            family: sources.family,
            directory,
            environment,
            variant: sources.variant,
            inputs: sources.inputs,
            artefacts,
        }
    }

    /// Describe a family whose nested builds all run against `directory`.
    ///
    /// A family's code path sets `CARGO_TARGET_DIR` to exactly the directory
    /// it stamps, so recording that directory as the child's target is what
    /// keeps the stamp and the artefacts it vouches for in step. The rest of
    /// the inherited environment is filtered by the shared rules, with the
    /// caller's profile destination preserved.
    ///
    /// `inputs` is the caller-supplied hash of the fixture sources; callers
    /// derive it before preparation runs, because preparation writes inside
    /// the very roots it hashes.
    /// Coverage stays with the caller: the fixture's own run is what the
    /// coverage gate measures, so an inherited `LLVM_PROFILE_FILE` is kept.
    #[must_use]
    pub fn for_fixture(
        family: impl Into<String>,
        directory: PathBuf,
        variant: impl Into<String>,
        inputs: impl Into<String>,
    ) -> Self {
        let environment = env_for_directory(&directory, ProfileDestination::Caller);
        Self::new(
            Sources::new(family, variant, inputs),
            directory,
            environment,
        )
    }
}

/// Filter the inherited environment for a child that builds in `directory`.
///
/// `CARGO_TARGET_DIR` is dropped from the inherited values because the
/// directory being stamped is the authority: a stale inherited value would
/// point the build somewhere the stamp does not describe.
#[must_use]
pub fn env_for_directory(
    directory: &Path,
    profile_destination: ProfileDestination,
) -> NestedCargoEnv {
    env_from_vars(
        env::vars_os()
            .filter(|(key, _)| key != "CARGO_TARGET_DIR")
            .collect(),
        Some(directory),
        profile_destination,
    )
}

/// What one family's preparation did, for the log line.
#[derive(Debug, Default, Clone, Copy)]
pub struct PreparationSummary {
    /// Number of `Compiling` lines the nested builds emitted.
    pub compiled_units: usize,
}

/// Ensure the family's artefacts are prepared, running `prepare` if needed.
///
/// A stamp that matches the current fingerprint returns immediately. A missing
/// or stale stamp under `cargo nextest` returns a [`io::ErrorKind::NotFound`]
/// error naming the preparation command instead of preparing inside a timed
/// test. Under plain `cargo test` the closure runs under a process-wide lock,
/// and the stamp is written only after it succeeds.
///
/// # Errors
///
/// Returns the closure's error when preparation fails, and the I/O error when
/// the stamp cannot be read or written.
pub fn ensure_prepared<F>(preparation: &Preparation, prepare: F) -> io::Result<()>
where
    F: FnOnce() -> io::Result<PreparationSummary>,
{
    ensure_prepared_with(preparation, running_under_nextest(), prepare)
}

/// [`ensure_prepared`] with the runner decision supplied.
///
/// Exposed as hidden API so the protocol can be tested — here and in the
/// workspace suites that gate on it — without mutating the process
/// environment, which would race every other test in the same binary.
///
/// # Errors
///
/// As [`ensure_prepared`].
#[doc(hidden)]
pub fn ensure_prepared_with<F>(
    preparation: &Preparation,
    under_nextest: bool,
    prepare: F,
) -> io::Result<()>
where
    F: FnOnce() -> io::Result<PreparationSummary>,
{
    let fingerprint = fingerprint(preparation);
    let stamp = stamp_path(preparation);
    if stamp_matches(&stamp, &fingerprint) && artefacts_are_present(preparation) {
        note(&format!(
            "family={} is prepared ({})",
            preparation.family,
            stamp.display()
        ));
        return Ok(());
    }

    if under_nextest {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            missing_stamp_message(preparation, &stamp),
        ));
    }

    prepare_locked(preparation, &stamp, &fingerprint, prepare)
}

/// Prepare the family unconditionally, for the named preparation test.
///
/// This is the entry point `make prepare-nested-fixtures` reaches: it runs
/// under nextest, where [`ensure_prepared`] would otherwise fail fast. It is
/// still idempotent, so a warm target directory costs only the stamp read.
///
/// # Errors
///
/// As [`ensure_prepared`].
pub fn prepare_now<F>(preparation: &Preparation, prepare: F) -> io::Result<()>
where
    F: FnOnce() -> io::Result<PreparationSummary>,
{
    let fingerprint = fingerprint(preparation);
    prepare_locked(preparation, &stamp_path(preparation), &fingerprint, prepare)
}

/// Whether every directory the stamp vouches for still holds its artefacts.
///
/// A matching stamp answers "the inputs and environment have not changed since
/// this family was prepared". It cannot answer "the compiled units are still
/// there"; `artefacts` answers that, and looks one level deeper than the
/// profile directory itself — see [`DEPENDENCY_ARTEFACT_DIRS`] for why an
/// existing profile is not evidence that its dependency graph survived.
fn artefacts_are_present(preparation: &Preparation) -> bool { artefacts::all_present(preparation) }

/// Run `prepare` under the process-wide lock unless the family is already
/// prepared, then stamp it.
fn prepare_locked<F>(
    preparation: &Preparation,
    stamp: &Path,
    fingerprint: &str,
    prepare: F,
) -> io::Result<()>
where
    F: FnOnce() -> io::Result<PreparationSummary>,
{
    let _guard = lock();
    // Another process or thread may have prepared the family while this one
    // waited for the lock. The artefacts are re-checked on the same terms as
    // the gate: a stamp whose tree was evicted while this process waited is
    // not a prepared family.
    if stamp_matches(stamp, fingerprint) && artefacts_are_present(preparation) {
        return Ok(());
    }

    let started = Instant::now();
    let summary = prepare()?;
    write_stamp(stamp, fingerprint)?;
    note(&format!(
        "family={} prepared in {:.1}s ({} Compiling lines); stamp {}",
        preparation.family,
        started.elapsed().as_secs_f64(),
        summary.compiled_units,
        stamp.display()
    ));
    Ok(())
}

/// Count `Compiling` lines in captured child output.
#[must_use]
pub fn count_compiling(stderr: &[u8]) -> usize {
    String::from_utf8_lossy(stderr)
        .lines()
        .filter(|line| line.trim_start().starts_with("Compiling "))
        .count()
}

/// Whether the current process was started by `cargo nextest`.
#[must_use]
pub fn running_under_nextest() -> bool { env::var_os("NEXTEST_RUN_ID").is_some() }

/// Take the process-wide preparation lock.
fn lock() -> MutexGuard<'static, ()> { PREPARE_LOCK.lock().unwrap_or_else(PoisonError::into_inner) }
