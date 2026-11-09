//! Behavioural coverage for the stamp protocol and runner policy.
//!
//! The cases are split by what they exercise: `stamping` covers the
//! preparation lifecycle and the gate that admits a timed test against it,
//! `eviction` covers the states a stamp cannot see because only the profile
//! beneath it was removed or gutted, `inputs` covers the environment and input
//! hashing that decide when a prepared directory is stale, and `cargo_config`
//! covers Cargo's own configuration as one of those inputs. The fixtures all
//! four share live here.

mod cargo_config;
mod eviction;
mod inputs;
mod stamping;

use std::path::{Path, PathBuf};

use super::Preparation;
use crate::nested_cargo::{DEPENDENCY_ARTEFACT_DIRS, NestedCargoEnv, ProfileDestination};

/// A fixed environment for tests that only exercise fingerprint composition.
///
/// The target directory is synthetic: these cases never consult the stamp, so
/// it need not exist. It is expressed through [`std::env::temp_dir`] rather than
/// as a Unix literal so that it is absolute on every platform: a literal such
/// as `/tmp/target` has no drive letter on Windows, making it a *relative*
/// path, and a fixture whose meaning changes with the host cannot describe one
/// rule.
fn environment() -> NestedCargoEnv { environment_in(&std::env::temp_dir().join("target")) }

/// Populate a profile directory the way a real Cargo build leaves it.
///
/// The gate requires more than an existing `<target>/debug`: it requires the
/// `deps` and `.fingerprint` entries Cargo reads to decide its units are
/// current, and that every dep-info record in `deps` still has the output it
/// describes. A fixture that created only the profile, or only a bare `deps`
/// directory, would be accepted by a weaker check and would prove nothing
/// about the one under test.
///
/// `deps` is filled by [`write_unit`], not by a placeholder: any file whose
/// name equals a dep-info stem would be indistinguishable from that unit's
/// linked binary, so a fixture that wrote one would make the pairing predicate
/// look satisfied for the wrong reason.
fn populate_profile(target: &Path) {
    let profile = target.join(super::DEFAULT_PROFILE);
    let [deps, fingerprints] = DEPENDENCY_ARTEFACT_DIRS.map(|name| profile.join(name));
    std::fs::create_dir_all(&deps).expect("deps dir");
    std::fs::create_dir_all(&fingerprints).expect("fingerprint dir");
    std::fs::write(fingerprints.join("unit"), b"fingerprint record").expect("fingerprint record");
    write_unit(target, "unit", "rlib");
    // A unit built for `cargo check`, or one only ever asked for metadata,
    // leaves no `rlib` at all — only this. It is the commonest shape in a real
    // `deps` directory, so the fixture must carry it or the gate would pass
    // here and refuse a genuine tree.
    write_unit(target, "checked", "rmeta");
}

/// Write one compiled unit the way rustc does: a `.d` record and its output.
///
/// `stem` is the unit's identity and `extension` is the form rustc left —
/// `rlib` for a linked library, `rmeta` for a metadata-only unit. Writing both
/// halves is what a real build leaves behind, and the gate pairs them, so a
/// fixture that wrote only the record would describe a tree Cargo cannot
/// reuse.
fn write_unit(target: &Path, stem: &str, extension: &str) {
    let deps = target.join(super::DEFAULT_PROFILE).join("deps");
    std::fs::create_dir_all(&deps).expect("deps dir");
    std::fs::write(deps.join(format!("{stem}.d")), b"dep-info").expect("dep-info");
    std::fs::write(deps.join(format!("lib{stem}.{extension}")), b"unit").expect("unit output");
}

/// A filtered environment whose target directory is `target`.
///
/// The target directory must exist for the gate to accept a matching stamp:
/// the profile directory it derives must be present and hold the dependency
/// artefacts, so a scratch preparation creates both.
fn environment_in(target: &Path) -> NestedCargoEnv {
    crate::nested_cargo::env_from_vars(
        vec![
            ("RUSTFLAGS".into(), "-Dwarnings".into()),
            ("NEXTEST_RUN_ID".into(), "run-1".into()),
            (
                "LLVM_PROFILE_FILE".into(),
                std::env::temp_dir().join("parent.profraw").into_os_string(),
            ),
            ("PATH".into(), "/usr/bin".into()),
        ],
        Some(target),
        ProfileDestination::Caller,
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
        super::Sources::new("unit", variant, inputs),
        directory.to_path_buf(),
        environment.clone(),
    )
}

/// A scratch preparation with variant `"v"` and input hash `"hash"`.
///
/// The `TempDir` is returned alongside so the caller keeps it alive; the
/// preparation's directory is the temporary directory itself, and its target
/// directory is a `target/` child that this helper creates along with the
/// profile directory the gate requires.
fn scratch_preparation() -> (tempfile::TempDir, Preparation, NestedCargoEnv) {
    let directory = tempfile::tempdir().expect("temp dir");
    let target = directory.path().join("target");
    populate_profile(&target);
    let environment = environment_in(&target);
    let preparation = preparation(directory.path(), &environment, "v", "hash");
    (directory, preparation, environment)
}

/// A scratch root holding a minimal fixture package.
///
/// Every behavioural case needs the same starting point: a temporary
/// directory with a `fixture/` package inside it. The `TempDir` is returned so
/// the caller keeps it alive for the duration of the test; dropping it removes
/// the tree.
fn fixture_package() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("temp dir");
    let source = directory.path().join("fixture");
    std::fs::create_dir_all(&source).expect("fixture dir");
    std::fs::write(source.join("Cargo.toml"), "[package]\nname = \"f\"\n").expect("manifest");
    (directory, source)
}
