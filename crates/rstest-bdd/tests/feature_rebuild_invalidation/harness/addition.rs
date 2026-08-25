//! Milestone 7's build-script addition experiment (`ExecPlan` 10.3.3).
//!
//! Closes the file-*addition* gap that per-file tracking cannot see: the
//! `feature_addition` fixture binds a directory with `scenarios!` and has
//! **no** committed `build.rs`. This experiment writes the `build.rs`
//! extracted from the *tested living documentation* example in
//! `docs/users-guide.md` (marker `scenarios-build-script`), adds a
//! `build = "build.rs"` key to the fixture's manifest, runs `cargo test`
//! (the baseline scenario must run), adds a brand-new `.feature` file to the
//! bound directory, and re-runs `cargo test` — the new scenario appearing in
//! the output proves the recipe works, so a recipe that stops working fails
//! the suite instead of rotting silently.

use std::{
    sync::OnceLock,
    time::{Duration, SystemTime},
};

use super::{fixtures, process};
use crate::documentation_examples::documented_example;

/// Outcome of the build-script addition experiment.
pub(crate) struct AdditionOutcome {
    /// A human-readable reason when setup or the baseline run failed.
    pub(crate) baseline_error: Option<String>,
    /// The baseline run's full output, for failure reporting.
    pub(crate) baseline_output: String,
    /// Whether the post-addition run's output names the new scenario's test.
    pub(crate) new_scenario_ran: bool,
    /// The post-addition run's full output, for failure reporting.
    pub(crate) second_run_output: String,
    /// Whether the post-addition run recompiled the fixture.
    pub(crate) second_run_recompiled: bool,
}

static ADDITION: OnceLock<AdditionOutcome> = OnceLock::new();

/// Run the build-script addition experiment, once per process.
pub(crate) fn addition_outcome() -> &'static AdditionOutcome {
    ADDITION.get_or_init(run_addition_experiment)
}

/// The `Then` step's captured value in the fixture's feature file before the
/// experiment edits it, and the value the edit switches it to.
const ADDED_FILE_NAME: &str = "zzz_added.feature";

/// The name of the generated test for the added scenario (feature stem
/// `zzz_added` plus the sanitized scenario title).
const ADDED_TEST_NAME: &str = "zzz_added_the_added_scenario";

fn run_addition_experiment() -> AdditionOutcome {
    fixtures::ensure_addition_fixture_copied();
    reset_added_file();
    if let Err(err) = write_build_script_from_docs() {
        return AdditionOutcome {
            baseline_error: Some(err),
            baseline_output: String::new(),
            new_scenario_ran: false,
            second_run_output: String::new(),
            second_run_recompiled: false,
        };
    }
    add_build_key_to_manifest();

    let env = process::build_child_env();
    let outcome = run_cargo(&env, &["test", "--locked", "--offline"]);
    let (baseline_error, baseline_output) = match outcome {
        Ok(captured) => (None, captured.stdout),
        Err(err) => (
            Some(format!("baseline cargo test failed to run: {err}")),
            String::new(),
        ),
    };
    if let Some(reason) = baseline_error.clone() {
        return AdditionOutcome {
            baseline_error: Some(reason),
            baseline_output,
            new_scenario_ran: false,
            second_run_output: String::new(),
            second_run_recompiled: false,
        };
    }

    if let Err(err) = add_new_feature_file() {
        return AdditionOutcome {
            baseline_error: Some(format!("cannot add the new feature file: {err}")),
            baseline_output,
            new_scenario_ran: false,
            second_run_output: String::new(),
            second_run_recompiled: false,
        };
    }

    let second_captured = run_cargo(&env, &["test", "--locked", "--offline"]);
    let second_captured = match second_captured {
        Ok(captured) => captured,
        Err(err) => panic!("second cargo test failed to run: {err}"),
    };
    let new_scenario_ran = second_captured.stdout.contains(ADDED_TEST_NAME);
    let second_run_recompiled = second_captured
        .stdout
        .contains("Compiling rstest-bdd-feature-addition-fixture");

    AdditionOutcome {
        baseline_error: None,
        baseline_output,
        new_scenario_ran,
        second_run_output: second_captured.stdout,
        second_run_recompiled,
    }
}

/// Run a nested `cargo` invocation with the shared environment from the
/// addition fixture directory, returning its combined output.
fn run_cargo(env: &process::ChildEnv, args: &[&str]) -> Result<process::Captured, String> {
    let mut cmd = process::cargo_command(env, &fixtures::scratch_addition_dir());
    cmd.args(args);
    process::run_bounded(&mut cmd)
        .map(process::Captured::from)
        .map_err(|err| err.to_string())
}

/// Write the fixture's `build.rs` from the extracted documentation example.
///
/// The example is the single source of truth: if the recipe in
/// `docs/users-guide.md` stops working, this extraction fails (or the
/// executed recipe fails the behavioural test).
fn write_build_script_from_docs() -> Result<(), String> {
    let example = documented_example("scenarios-build-script")
        .map_err(|err| format!("cannot load the documented build-script recipe: {err}"))?;
    if example.language.as_str() != "rust" {
        return Err(format!(
            "the `scenarios-build-script` example must be a `rust` fence, got `{}`",
            example.language.as_str()
        ));
    }
    let build_rs = fixtures::scratch_addition_dir().join("build.rs");
    std::fs::write(&build_rs, example.body)
        .map_err(|err| format!("cannot write {}: {err}", build_rs.display()))
}

/// Add `build = "build.rs"` to the fixture's `[package]` table when absent.
fn add_build_key_to_manifest() {
    let manifest_path = fixtures::scratch_addition_dir().join("Cargo.toml");
    let text = match std::fs::read_to_string(&manifest_path) {
        Ok(text) => text,
        Err(err) => panic!("cannot read addition manifest: {err}"),
    };
    if text.contains("build = \"build.rs\"") {
        return;
    }
    let Some((before_deps, after_deps)) = text.split_once("[dependencies]") else {
        panic!("addition manifest has no [dependencies] table");
    };
    let rewritten = format!("{before_deps}build = \"build.rs\"\n\n[dependencies]{after_deps}");
    if let Err(err) = std::fs::write(&manifest_path, rewritten) {
        panic!("cannot write addition manifest: {err}");
    }
}

/// Remove a leftover added file so every run observes the same baseline.
fn reset_added_file() {
    let path = added_file_path();
    if path.exists() {
        if let Err(err) = std::fs::remove_file(&path) {
            panic!(
                "cannot remove leftover added file {}: {err}",
                path.display()
            );
        }
    }
}

fn added_file_path() -> std::path::PathBuf {
    fixtures::scratch_addition_dir()
        .join("tests/features")
        .join(ADDED_FILE_NAME)
}

/// Create the new `.feature` file with an mtime far in the future so the
/// filesystem timestamp is never ambiguous.
fn add_new_feature_file() -> Result<(), String> {
    let path = added_file_path();
    let content = "Feature: The added feature\n\n  Scenario: The added scenario\n    Given a \
                   directory-bound step\n";
    std::fs::write(&path, content)
        .map_err(|err| format!("cannot write {}: {err}", path.display()))?;
    // On Windows, drop the write handle before reopening to set the time.
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .map_err(|err| format!("cannot reopen {}: {err}", path.display()))?;
    let future = SystemTime::now() + Duration::from_secs(2);
    file.set_modified(future)
        .map_err(|err| format!("cannot set future mtime on {}: {err}", path.display()))
} // `file` is dropped here, before the next `cargo test` reads the marker.
