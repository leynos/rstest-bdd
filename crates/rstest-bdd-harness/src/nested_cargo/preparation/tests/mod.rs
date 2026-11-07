//! Behavioural coverage for the stamp protocol and runner policy.
//!
//! The cases are split by what they exercise: `stamping` covers the
//! preparation lifecycle and the gate that admits a timed test against it, and
//! `inputs` covers the environment and input hashing that decide when a
//! prepared directory is stale. The fixtures both share live here.

mod inputs;
mod stamping;

use std::path::{Path, PathBuf};

use super::Preparation;
use crate::nested_cargo::{DEPENDENCY_ARTEFACT_DIRS, NestedCargoEnv, ProfileDestination};

/// A fixed environment for tests that only exercise fingerprint composition.
///
/// The target directory is synthetic: these cases never consult the stamp, so
/// it need not exist.
fn environment() -> NestedCargoEnv { environment_in(Path::new("/tmp/target")) }

/// Populate a profile directory the way a real Cargo build leaves it.
///
/// The gate requires more than an existing `<target>/debug`: it requires the
/// `deps` and `.fingerprint` entries Cargo reads to decide its units are
/// current. A fixture that created only the profile would be accepted by a
/// weaker check and would prove nothing about the one under test.
fn populate_profile(target: &Path) {
    for name in DEPENDENCY_ARTEFACT_DIRS {
        let directory = target.join(super::DEFAULT_PROFILE).join(name);
        std::fs::create_dir_all(&directory).expect("dependency artefact dir");
        std::fs::write(directory.join("unit"), b"artefact").expect("dependency artefact");
    }
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
            ("LLVM_PROFILE_FILE".into(), "/tmp/parent.profraw".into()),
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
