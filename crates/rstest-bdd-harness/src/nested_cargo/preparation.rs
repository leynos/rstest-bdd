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
//! * under plain `cargo test` preparation runs lazily under a process-wide lock, so the same suites
//!   stay usable without the extra Nextest pass. `#[serial]` (which locks across processes) remains
//!   the caller's responsibility for test binaries that share a family.
//!
//! Preparation commands are bounded by [`PREPARATION_COMMAND_TIMEOUT`], which
//! is deliberately separate from every assertion deadline.

use std::{
    env,
    fmt::Write as _,
    fs,
    hash::{Hash as _, Hasher},
    io,
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, MutexGuard, OnceLock, PoisonError},
    time::{Duration, Instant},
};

use super::NestedCargoEnv;

/// Wall-clock bound for one nested preparation command.
///
/// A preparation command compiles what the timed tests would otherwise
/// compile inside their own deadlines, so its bound is sized for a queued cold
/// build on a two-vCPU CI runner rather than for a warm assertion. It is not,
/// and must not become, the deadline of any test.
pub const PREPARATION_COMMAND_TIMEOUT: Duration = Duration::from_secs(45 * 60);

/// Fingerprint format version; bump to invalidate every existing stamp.
const STAMP_VERSION: &str = "1";

/// Suffix of the hidden stamp file written inside a prepared artefact
/// directory.
const PREPARED_FILE_SUFFIX: &str = ".rstest-bdd-prepared";

/// Process-wide lock serialising lazy preparation under plain `cargo test`.
static PREPARE_LOCK: Mutex<()> = Mutex::new(());

/// Environment variables that change what a nested preparation builds.
///
/// Everything else in the child environment (`NEXTEST_*`,
/// `LLVM_PROFILE_FILE`, thread counts, logging filters) is excluded: a change
/// to those must not force a rebuild.
const FINGERPRINT_EXACT: [&str; 5] = [
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_INCREMENTAL",
    "RUSTDOCFLAGS",
    "RUSTUP_TOOLCHAIN",
];

/// Prefixes whose whole variable set changes the compiled artefacts.
const FINGERPRINT_PREFIXES: [&str; 5] = [
    "RUSTC",
    "RUSTDOC",
    "CARGO_BUILD_",
    "CARGO_PROFILE_",
    "CARGO_TARGET_",
];

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

/// Hash the inputs a preparation family's artefacts derive from.
///
/// `roots` may name files or directories; directories are walked in sorted
/// order and every regular file contributes its relative path, length and
/// bytes. Directory entries named `target` or `.git` are skipped, as are the
/// stamp files this module writes, so a nested build never invalidates the
/// fingerprint that describes it.
///
/// # Errors
///
/// Returns the read error when a named input or one of its files cannot be
/// read.
pub fn input_hash(roots: &[PathBuf]) -> io::Result<String> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    STAMP_VERSION.hash(&mut hasher);
    for root in roots {
        hash_entry(root, &mut hasher)?;
    }
    Ok(format!("{:016x}", hasher.finish()))
}

/// Feed one file or directory tree into `hasher`.
fn hash_entry(path: &Path, hasher: &mut impl Hasher) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_file() {
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .hash(hasher);
        let contents = fs::read(path)?;
        contents.hash(hasher);
        return Ok(());
    }
    if !metadata.is_dir() {
        return Ok(());
    }
    let mut entries: Vec<PathBuf> = fs::read_dir(path)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<_>>()?;
    entries.sort();
    for entry in entries {
        let name = entry
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        // `target` holds build output and the stamp names the fingerprint
        // computed from these inputs, so neither is an input.
        if name == "target" || name.ends_with(PREPARED_FILE_SUFFIX) {
            continue;
        }
        name.hash(hasher);
        hash_entry(&entry, hasher)?;
    }
    Ok(())
}

/// Whether the current process was started by `cargo nextest`.
#[must_use]
pub fn running_under_nextest() -> bool { env::var_os("NEXTEST_RUN_ID").is_some() }

/// Report preparation progress on standard error.
///
/// Preparation is intentionally visible in test logs: its elapsed time and
/// compiled-unit count are the evidence that the cold work left the timed
/// assertions.
fn note(message: &str) {
    use std::io::Write as _;
    let _ = writeln!(io::stderr(), "nested preparation: {message}");
}

/// Take the process-wide preparation lock.
fn lock() -> MutexGuard<'static, ()> { PREPARE_LOCK.lock().unwrap_or_else(PoisonError::into_inner) }

/// The stamp file protecting `preparation.directory`.
fn stamp_path(preparation: &Preparation) -> PathBuf {
    preparation
        .directory
        .join(format!(".{}{}", preparation.family, PREPARED_FILE_SUFFIX))
}

/// Whether the stamp exists and records exactly `fingerprint`.
fn stamp_matches(stamp: &Path, fingerprint: &str) -> bool {
    match fs::read_to_string(stamp) {
        Ok(recorded) => recorded.trim_end() == fingerprint.trim_end(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => {
            note(&format!(
                "could not read stamp {}: {error}; preparing again",
                stamp.display()
            ));
            false
        }
    }
}

/// Write the stamp atomically, so a killed preparation never looks complete.
fn write_stamp(stamp: &Path, fingerprint: &str) -> io::Result<()> {
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
    fs::write(&temporary, fingerprint)?;
    fs::rename(&temporary, stamp)
}

/// The fingerprint text recorded in the stamp.
fn fingerprint(preparation: &Preparation) -> String {
    let mut text = String::new();
    let _ = writeln!(text, "version: {STAMP_VERSION}");
    let _ = writeln!(text, "toolchain: {}", toolchain_identity());
    for (key, value) in preparation.environment.iter() {
        let key_text = key.to_string_lossy();
        if is_fingerprint_variable(&key_text) {
            let _ = writeln!(text, "{key_text}={}", value.to_string_lossy());
        }
    }
    let _ = writeln!(text, "variant: {}", preparation.variant);
    let _ = writeln!(text, "inputs: {}", preparation.inputs);
    text
}

/// Whether `key` influences the compiled artefacts.
fn is_fingerprint_variable(key: &str) -> bool {
    FINGERPRINT_EXACT.contains(&key)
        || FINGERPRINT_PREFIXES
            .iter()
            .any(|prefix| key.starts_with(prefix))
}

/// The fail-fast message for a timed test with no prepared artefacts.
fn missing_stamp_message(preparation: &Preparation, stamp: &Path) -> String {
    format!(
        "nested fixture artefacts for family `{}` are not prepared ({} is missing or \
         stale).\nUnder nextest every test runs in its own process, so preparing here would make \
         a timed test pay for the cold build it is meant to avoid.\nPrepare them first with `make \
         prepare-nested-fixtures` (or `cargo nextest run -E \
         'test(/nested_fixture_preparation$/)'`).",
        preparation.family,
        stamp.display()
    )
}

/// Identify the toolchain that produced the prepared artefacts.
///
/// `cargo -vV` reports the release, commit and host of the Cargo in use, which
/// changes whenever the toolchain does. Resolved from the `CARGO`
/// compile-time variable, never a `PATH` lookup.
fn toolchain_identity() -> &'static str {
    static IDENTITY: OnceLock<String> = OnceLock::new();
    IDENTITY.get_or_init(|| {
        let output = Command::new(env!("CARGO")).arg("-vV").output();
        let Ok(output) = output else {
            note("could not run `cargo -vV`; toolchain unknown");
            return "unknown".to_owned();
        };
        if !output.status.success() {
            note("`cargo -vV` failed; toolchain unknown");
            return "unknown".to_owned();
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let kept: Vec<&str> = text
            .lines()
            .filter(|line| {
                line.starts_with("release:")
                    || line.starts_with("commit-hash:")
                    || line.starts_with("host:")
            })
            .collect();
        if kept.is_empty() {
            "unknown".to_owned()
        } else {
            kept.join(" ")
        }
    })
}

#[cfg(test)]
mod tests {
    //! Behavioural coverage for the stamp protocol and runner policy.

    use super::*;

    /// A filtered environment carrying only fingerprint-relevant variables.
    fn environment() -> NestedCargoEnv {
        super::super::env_from_vars(
            vec![
                ("RUSTFLAGS".into(), "-Dwarnings".into()),
                ("NEXTEST_RUN_ID".into(), "run-1".into()),
                ("LLVM_PROFILE_FILE".into(), "/tmp/parent.profraw".into()),
                ("PATH".into(), "/usr/bin".into()),
            ],
            Some(Path::new("/tmp/target")),
            super::super::ProfileDestination::Caller,
        )
    }

    /// A preparation rooted at `directory`.
    fn preparation(
        directory: &Path,
        environment: &NestedCargoEnv,
        variant: &str,
        inputs: &str,
    ) -> Preparation {
        Preparation::new(
            "unit",
            directory.to_path_buf(),
            environment.clone(),
            variant,
            inputs,
        )
    }

    #[test]
    fn fingerprint_excludes_runner_and_coverage_variables() {
        let environment = environment();
        let text = fingerprint(&preparation(
            Path::new("/tmp/target"),
            &environment,
            "all-features",
            "abc",
        ));
        assert!(text.contains("RUSTFLAGS=-Dwarnings"));
        assert!(text.contains("CARGO_TARGET_DIR=/tmp/target"));
        assert!(text.contains("variant: all-features"));
        assert!(text.contains("inputs: abc"));
        assert!(!text.contains("NEXTEST_RUN_ID"));
        assert!(!text.contains("LLVM_PROFILE_FILE"));
        assert!(!text.contains("PATH="));
    }

    #[test]
    fn fingerprint_changes_with_variant_and_inputs() {
        let environment = environment();
        let one = fingerprint(&preparation(
            Path::new("/tmp/target"),
            &environment,
            "all-features",
            "abc",
        ));
        let other = fingerprint(&preparation(
            Path::new("/tmp/target"),
            &environment,
            "no-default-features",
            "abc",
        ));
        let third = fingerprint(&preparation(
            Path::new("/tmp/target"),
            &environment,
            "all-features",
            "def",
        ));
        assert_ne!(one, other);
        assert_ne!(one, third);
    }

    #[test]
    fn lazy_preparation_writes_the_stamp_and_is_idempotent() {
        let directory = tempfile::tempdir().expect("temp dir");
        let environment = environment();
        let preparation = preparation(directory.path(), &environment, "v", "hash");
        let mut runs = 0_u32;

        for _ in 0..2 {
            ensure_prepared_with(&preparation, false, || {
                runs += 1;
                Ok(PreparationSummary { compiled_units: 3 })
            })
            .expect("lazy preparation should succeed");
        }

        assert_eq!(runs, 1, "the second call should see the stamp");
        assert!(stamp_path(&preparation).is_file());
    }

    #[test]
    fn prepare_now_runs_under_nextest_and_stamps_once() {
        let directory = tempfile::tempdir().expect("temp dir");
        let environment = environment();
        let preparation = preparation(directory.path(), &environment, "v", "hash");
        let mut runs = 0_u32;
        for _ in 0..2 {
            prepare_now(&preparation, || {
                runs += 1;
                Ok(PreparationSummary { compiled_units: 1 })
            })
            .expect("explicit preparation should succeed");
        }
        assert_eq!(runs, 1, "a warm family should not prepare twice");
    }

    #[test]
    fn failed_preparation_leaves_no_stamp() {
        let directory = tempfile::tempdir().expect("temp dir");
        let environment = environment();
        let preparation = preparation(directory.path(), &environment, "v", "hash");
        let error = ensure_prepared_with(&preparation, false, || {
            Err(io::Error::other("nested build failed"))
        })
        .expect_err("preparation should fail");
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert!(!stamp_path(&preparation).exists());
    }

    #[test]
    fn nextest_refuses_to_prepare_and_names_the_command() {
        let directory = tempfile::tempdir().expect("temp dir");
        let environment = environment();
        let preparation = preparation(directory.path(), &environment, "v", "hash");
        let error = ensure_prepared_with(&preparation, true, || {
            panic!("nextest must not run preparation in a timed test")
        })
        .expect_err("a missing stamp under nextest must fail");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        let message = error.to_string();
        assert!(
            message.contains("make prepare-nested-fixtures"),
            "{message}"
        );
        assert!(message.contains("nested_fixture_preparation"), "{message}");
    }

    #[test]
    fn stale_fingerprint_forces_preparation() {
        let directory = tempfile::tempdir().expect("temp dir");
        let environment = environment();
        let first = preparation(directory.path(), &environment, "v", "hash");
        ensure_prepared_with(&first, false, || Ok(PreparationSummary::default()))
            .expect("initial preparation");

        let changed = preparation(directory.path(), &environment, "v", "changed-hash");
        let error = ensure_prepared_with(&changed, true, || {
            panic!("a stale stamp must not count as prepared")
        })
        .expect_err("stale inputs must fail under nextest");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn input_hash_ignores_stamps_and_build_output() {
        let directory = tempfile::tempdir().expect("temp dir");
        let source = directory.path().join("fixture");
        fs::create_dir_all(source.join("features")).expect("fixture dir");
        fs::write(source.join("Cargo.toml"), "[package]\nname = \"f\"\n").expect("manifest");
        fs::write(source.join("features/a.feature"), "Feature: a\n").expect("feature");
        let before = input_hash(std::slice::from_ref(&source)).expect("initial hash");

        fs::create_dir_all(source.join("target/debug")).expect("target dir");
        fs::write(source.join("target/debug/artifact"), "binary").expect("artefact");
        fs::write(
            source.join(format!(".unit{PREPARED_FILE_SUFFIX}")),
            "different fingerprint",
        )
        .expect("stamp");
        let after = input_hash(std::slice::from_ref(&source)).expect("hash after build");
        assert_eq!(before, after, "build output must not change the inputs");

        fs::write(source.join("features/a.feature"), "Feature: changed\n").expect("edit");
        let changed = input_hash(&[source]).expect("hash after edit");
        assert_ne!(before, changed, "fixture edits must change the inputs");
    }

    #[test]
    fn compiling_lines_are_counted_from_stderr() {
        let stderr = b"   Compiling foo v0.1.0\nwarning: unused\n    Finished `dev` profile\n   \
                       Compiling bar v0.2.0\n";
        assert_eq!(count_compiling(stderr), 2);
    }
}
