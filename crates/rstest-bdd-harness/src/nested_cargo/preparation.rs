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

mod input_hash;
mod stamp;

#[cfg(test)]
mod tests;

use std::{
    env,
    io,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

pub use input_hash::input_hash;
use stamp::{fingerprint, missing_stamp_message, note, stamp_matches, stamp_path, write_stamp};

use super::NestedCargoEnv;

/// Wall-clock bound for one nested preparation command.
///
/// A preparation command compiles what the timed tests would otherwise
/// compile inside their own deadlines, so its bound is sized for a queued cold
/// build on a two-vCPU CI runner rather than for a warm assertion. It is not,
/// and must not become, the deadline of any test.
pub const PREPARATION_COMMAND_TIMEOUT: Duration = Duration::from_secs(45 * 60);

/// Process-wide lock serializing lazy preparation under plain `cargo test`.
static PREPARE_LOCK: Mutex<()> = Mutex::new(());

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
}

impl Preparation {
    /// Build a preparation description.
    #[must_use]
    pub fn new(
        family: impl Into<String>,
        directory: PathBuf,
        environment: NestedCargoEnv,
        variant: impl Into<String>,
        inputs: impl Into<String>,
    ) -> Self {
        Self {
            family: family.into(),
            directory,
            environment,
            variant: variant.into(),
            inputs: inputs.into(),
        }
    }
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

/// [`ensure_prepared`] with the runner decision supplied, so the fail-fast and
/// lazy paths are testable without mutating the process environment.
fn ensure_prepared_with<F>(
    preparation: &Preparation,
    under_nextest: bool,
    prepare: F,
) -> io::Result<()>
where
    F: FnOnce() -> io::Result<PreparationSummary>,
{
    let fingerprint = fingerprint(preparation);
    let stamp = stamp_path(preparation);
    if stamp_matches(&stamp, &fingerprint) {
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
    // waited for the lock.
    if stamp_matches(stamp, fingerprint) {
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
