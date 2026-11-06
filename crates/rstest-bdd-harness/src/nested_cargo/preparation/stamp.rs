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
//! The stamp is written last, after every preparation command succeeded, and
//! replaced atomically through a temporary sibling, so a killed preparation
//! never looks complete.

use std::{
    env,
    fmt::Write as _,
    fs,
    io,
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

use super::Preparation;

/// Fingerprint format version; bump to invalidate every existing stamp.
pub(super) const STAMP_VERSION: &str = "1";

/// Suffix of the hidden stamp file written inside a prepared artefact
/// directory.
pub(super) const PREPARED_FILE_SUFFIX: &str = ".rstest-bdd-prepared";

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

/// The stamp file protecting `preparation.directory`.
pub(super) fn stamp_path(preparation: &Preparation) -> PathBuf {
    preparation
        .directory
        .join(format!(".{}{}", preparation.family, PREPARED_FILE_SUFFIX))
}

/// Whether the stamp exists and records exactly `fingerprint`.
pub(super) fn stamp_matches(stamp: &Path, fingerprint: &str) -> bool {
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
pub(super) fn write_stamp(stamp: &Path, fingerprint: &str) -> io::Result<()> {
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
pub(super) fn fingerprint(preparation: &Preparation) -> String {
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
pub(super) fn missing_stamp_message(preparation: &Preparation, stamp: &Path) -> String {
    format!(
        "nested fixture artefacts for family `{}` are not prepared ({} is missing or \
         stale).\nUnder nextest every test runs in its own process, so preparing here would make \
         a timed test pay for the cold build it is meant to avoid.\nPrepare them first with `make \
         prepare-nested-fixtures` (or `cargo nextest run -E \
         'test(/nested_fixture_preparation$/)'`), under the same `RUSTFLAGS` and feature \
         selection as the run that reads this stamp: the fingerprint covers both, so preparing \
         under a different environment only warms a tree this run will not reuse.",
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

/// Report preparation progress on standard error.
///
/// Preparation is intentionally visible in test logs: its elapsed time and
/// compiled-unit count are the evidence that the cold work left the timed
/// assertions.
pub(super) fn note(message: &str) {
    use std::io::Write as _;
    let _ = writeln!(io::stderr(), "nested preparation: {message}");
}
