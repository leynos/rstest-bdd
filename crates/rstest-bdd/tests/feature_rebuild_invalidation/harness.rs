//! Shared harness for the 10.3.3 feature-file rebuild-invalidation
//! regression tests (roadmap item 10.3.3).
//!
//! Both regression scenarios in the enclosing test binary bind the same
//! experiment: a hermetic nested `cargo` invocation over a scratch copy of
//! `tests/fixtures/rebuild_invalidation`, run against the shared workspace
//! `target/` so its path-dependency units are already warm.
//!
//! The scenario-test foot-gun this file guards against is real: a bug in the
//! `#[scenario]` macro could mask the very regression the scenario proves.
//! That is why the dep-info assertion is a direct filesystem check (rustc's
//! `.d` file next to the compiled test binary) rather than a macro-mediated
//! one — it is the primary contract; the rebuild-behaviour assertions are the
//! end-to-end corroboration.
//!
//! Environment hygiene matters twice over: the two invocations in the rebuild
//! experiment must use **byte-identical** child environments (a rebuild
//! triggered by an environment difference between run 1 and run 2 is the
//! single most likely spurious-red cause), and the child must neither stall
//! on an inherited Cargo jobserver nor pollute the parent's gated coverage
//! profile.
//!
//! Everything here is owned scratch: the checked-in fixture is never mutated
//! (see the stamp-file protocol in `fixtures`) and `rm -rf
//! target/tests/rebuild-invalidation` is always a safe manual reset.
//!
//! The module is split into focused helpers: `fixtures` owns the scratch-copy
//! and stamp protocol plus the manifest rewrite; `process` owns the child
//! environment and the bounded nested-cargo invocation; `outcome` owns the
//! result shapes the scenario steps assert on.

#[path = "harness/addition.rs"]
mod addition;
#[path = "harness/fixtures.rs"]
mod fixtures;
#[path = "harness/outcome.rs"]
mod outcome;
#[path = "harness/process.rs"]
mod process;

use std::{path::PathBuf, sync::OnceLock};

pub(crate) use addition::addition_outcome;
pub(crate) use outcome::{DepInfoOutcome, RebuildOutcome};

static DEP_INFO: OnceLock<DepInfoOutcome> = OnceLock::new();

static REBUILD: OnceLock<RebuildOutcome> = OnceLock::new();

/// The `Then` step's captured value in the fixture's feature file before the
/// experiment edits it, and the value the edit switches it to.
const ORIGINAL_EXPECTATION: u32 = 100;

const EDITED_EXPECTATION: u32 = 101;

/// The `Then` step line the rebuild test rewrites (value only — never the
/// step keyword or pattern text; two CI legs compile under
/// `strict-compile-time-validation`).
const THEN_STEP_PREFIX: &str = "Then the bound expectation is ";

/// Build the fixture and read its dep-info, once per process.
pub(crate) fn dep_info_outcome() -> &'static DepInfoOutcome {
    DEP_INFO.get_or_init(build_dep_info_outcome)
}

fn build_dep_info_outcome() -> DepInfoOutcome {
    fixtures::ensure_fixture_copied();
    fixtures::restore_feature_file();
    let env = process::build_child_env();
    // Drop any fixture units compiled by an earlier (pre-tracking) era so the
    // dep-info read describes the current build, not a stale binary.
    clean_fixture_units(&env);
    let output = run_cargo(
        &env,
        &[
            "test",
            "--no-run",
            "--message-format=json",
            "--locked",
            "--offline",
        ],
    );
    let (stdout, baseline_error) = match output {
        Ok(captured) => (captured.stdout, None),
        Err(err) => (String::new(), Some(err)),
    };
    if let Some(baseline_error) = baseline_error {
        return outcome::DepInfoOutcome {
            dep_info_entry_count: 0,
            scenarios_no_match_tracked: false,
            dep_info_sample: stdout,
            child_env_detail: process::describe_env(&env),
            baseline_error: Some(baseline_error),
        };
    }

    // Cargo's `target/debug/.fingerprint/<pkg>-<hash>/dep-test-<name>` is the
    // *real* rebuild input; rustc's `.d` written next to the binary is a
    // stable proxy for it with the same hash suffix.
    let Some(executable) = process::locate_test_executable(&stdout).map(PathBuf::from) else {
        let error = format!(
            "no `compiler-artifact` with a test-binary `executable` was reported by `cargo test \
             --no-run --message-format=json`; child output:\n{stdout}"
        );
        return outcome::DepInfoOutcome {
            dep_info_entry_count: 0,
            scenarios_no_match_tracked: false,
            dep_info_sample: stdout,
            child_env_detail: process::describe_env(&env),
            baseline_error: Some(error),
        };
    };
    let dot_d = PathBuf::from(&executable).with_extension("d");
    let dep_content = match std::fs::read_to_string(&dot_d) {
        Ok(content) => content,
        Err(err) => panic!(
            "cannot read dep-info {} for {}: {err}",
            dot_d.display(),
            executable.display()
        ),
    };
    // rustc's make-style `.d` repeats every dependency in each rule block —
    // once for the `.d` target, once for the binary target, once in the
    // per-source mapping — so "exactly once" is asserted against the
    // **primary rule** (the `.d`'s own dependency list), which is the
    // canonical entry Cargo's fingerprinting consults. This pins the
    // one-binding-per-file property from Decision D0: rustc deduplicates
    // the list, so a future per-scenario binding must still be listed once.
    // (rustc emits the rule as a single line; if a future rustc ever wrapped
    // long dependency lists, the needle could land on a continuation line and
    // read 0 — CodeRabbit's one residual hypothetical — worth a comment, not
    // a workaround.)
    let primary_rule = dep_content.lines().next().unwrap_or_default();
    // `rustc` emits Windows paths with backslashes and preserves the drive
    // letter's case, while the expected path is normalized for stable
    // cross-platform comparison.
    let primary_rule = primary_rule.replace('\\', "/").to_lowercase();
    let expected = fixtures::normalize_dep_path(&fixtures::scratch_feature_file());
    let entry_count = primary_rule.split(&expected).count().saturating_sub(1);
    // The `scenarios!` directory's filtered-out file: it is parsed by the
    // macro (its scenarios are excluded by the `tags =` filter, generating no
    // tests), so it must still appear in dep-info for an edit to trigger a
    // rebuild.
    let no_match = fixtures::normalize_dep_path(
        &fixtures::scratch_fixture_dir().join("tests/features/scenarios_dir/no_match.feature"),
    );
    let scenarios_no_match_tracked = primary_rule.contains(&no_match);
    outcome::DepInfoOutcome {
        dep_info_entry_count: entry_count,
        scenarios_no_match_tracked,
        dep_info_sample: dep_content,
        child_env_detail: process::describe_env(&env),
        baseline_error: None,
    }
}

/// Run the full edit-and-rebuild experiment, once per process.
pub(crate) fn rebuild_outcome() -> &'static RebuildOutcome {
    REBUILD.get_or_init(build_rebuild_outcome)
}

fn build_rebuild_outcome() -> RebuildOutcome {
    fixtures::ensure_fixture_copied();
    // A previous run may have left the scratch feature file edited (and, while
    // the tracking mechanism is absent, a stale fixture binary compiled from
    // that edit lingers in the shared target). Reset the file to the
    // checked-in source and drop the fixture's compiled units so the baseline
    // always starts from the pristine feature text.
    fixtures::restore_feature_file();
    let env = process::build_child_env();
    clean_fixture_units(&env);

    // Baseline full run must pass.
    let baseline = run_cargo(&env, &["test", "--locked", "--offline"]);
    let (baseline_passed, baseline_output) = match baseline {
        Ok(captured) => (captured.status, captured.stdout),
        Err(err) => (false, format!("baseline cargo test failed to run: {err}")),
    };

    // Rewrite ONLY the captured value on the `Then` step, then push its mtime
    // two seconds into the future so the change is never masked by filesystem
    // timestamp granularity.
    let feature_path = fixtures::scratch_feature_file();
    let original = match std::fs::read_to_string(&feature_path) {
        Ok(text) => text,
        Err(err) => panic!("cannot read scratch feature file: {err}"),
    };
    let then_line_original = format!("{THEN_STEP_PREFIX}{ORIGINAL_EXPECTATION}");
    let then_line_edited = format!("{THEN_STEP_PREFIX}{EDITED_EXPECTATION}");
    let replaced = original.replace(&then_line_original, &then_line_edited);
    assert_ne!(
        replaced, original,
        "expected to find the `Then` step line `{then_line_original}` in the fixture's feature \
         file; the file has drifted from the contract the experiment relies on"
    );
    if let Err(err) = std::fs::write(&feature_path, &replaced) {
        panic!(
            "cannot rewrite scratch feature file {}: {err}",
            feature_path.display()
        );
    }
    // On Windows, drop the write handle before reopening to set the time.
    let file = match std::fs::OpenOptions::new().write(true).open(&feature_path) {
        Ok(file) => file,
        Err(err) => panic!(
            "cannot reopen scratch feature file {}: {err}",
            feature_path.display()
        ),
    };
    let future = std::time::SystemTime::now() + std::time::Duration::from_secs(2);
    if let Err(err) = file.set_modified(future) {
        panic!("cannot set the scratch feature mtime into the future: {err}");
    }
    drop(file);

    let second_captured = run_cargo(&env, &["test", "--locked", "--offline"]);
    let second_captured = match second_captured {
        Ok(captured) => captured,
        Err(err) => panic!("second cargo test failed to run: {err}"),
    };
    let failed = !second_captured.status;
    let recompiled = second_captured
        .stdout
        .contains("Compiling rstest-bdd-rebuild-invalidation-fixture");
    // The load-bearing check: the new expectation string exists only in the
    // new Gherkin text, so a failure message naming it proves the binary was
    // recompiled from that text.
    let names_new_expectation = second_captured
        .stdout
        .contains(&EDITED_EXPECTATION.to_string());

    outcome::RebuildOutcome {
        baseline_passed,
        baseline_output,
        second: outcome::SecondRun {
            failed,
            recompiled,
            names_new_expectation,
            output: second_captured.stdout,
        },
    }
}

/// Run a nested `cargo` invocation with the shared environment from the
/// scratch fixture directory, returning its combined output.
fn run_cargo(env: &process::ChildEnv, args: &[&str]) -> Result<process::Captured, String> {
    let mut cmd = process::cargo_command(env, &fixtures::scratch_fixture_dir());
    cmd.args(args);
    process::run_bounded(&mut cmd)
        .map(process::Captured::from)
        .map_err(|err| err.to_string())
}

/// Remove the fixture crate's compiled units from the shared build directory.
///
/// While the tracking mechanism is absent (pre-fix), Cargo cannot see a
/// `.feature`-only change, so a binary compiled from a previous experiment's
/// edited scratch would be reused forever. Cleaning the package's own units —
/// never its dependencies, which stay warm — guarantees the baseline compile
/// reflects the restored feature text.
fn clean_fixture_units(env: &process::ChildEnv) {
    let mut cmd = process::cargo_command(env, &fixtures::scratch_fixture_dir());
    cmd.args(["clean", "-p", "rstest-bdd-rebuild-invalidation-fixture"]);
    if let Err(err) = process::run_bounded(&mut cmd) {
        panic!("cargo clean of the fixture failed: {err}");
    }
}
