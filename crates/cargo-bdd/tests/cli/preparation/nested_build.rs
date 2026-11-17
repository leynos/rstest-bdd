//! The fixture's nested build, and the evidence it files for a checker.
//!
//! Preparation exists so the timed CLI tests never pay a cold dependency
//! compile. This module runs that build and records what it actually compiled,
//! in a form a reader outside the test process can observe. The two belong
//! together: a count nothing reports is indistinguishable from a count of zero,
//! which is the defect the report exists to close.

use eyre::{Context, Result};
use rstest_bdd_harness::nested_cargo;

use crate::{fixture_dir, locate_or_build_cargo_bdd_command};

/// Marker introducing the line that reports this family's nested build.
///
/// The preparation summary alone cannot prove a tree was reused.
/// [`nested_cargo::count_compiling`] turns the nested `cargo` output into a
/// number, and a number nothing reports is indistinguishable from zero. The
/// crate's own test helper also captures this process's stderr and drops it on
/// success, and nextest hides a passing test's output, so a reader of the run's
/// output could never see a rebuild at all — a scan of `Compiling` lines there
/// was reading lines nothing wrote.
///
/// The marker also separates the *nested* build's events from the outer one's:
/// `Compiling` lines in this process's output may belong to the outer Cargo
/// run, and counting those would report a rebuild that never happened.
const NESTED_BUILD_MARKER: &str = "cargo-bdd-nested-build:";

/// Environment variable naming a file to append this family's report to.
///
/// A file rather than a stream, because both carriers a test's output could
/// use drop it: the helper that spawns `cargo bdd steps` discards its capture
/// on success, and nextest suppresses a passing test's output. Naming a path in
/// the environment lets the caller choose somewhere durable — the workflow
/// driver points it inside its private target directory, so the evidence is
/// both visible and thrown away with the tree.
const REPORT_PATH: &str = "RSTEST_BDD_NESTED_BUILD_REPORT";

/// Run one diagnostic against the fixture, returning the captured output.
///
/// The child gets the filtered environment and an unfiltered inherited one for
/// `cargo`'s own diagnostics, matching how the timed tests spawn it except
/// that the output is bounded and captured.
fn capture_cargo_bdd_steps(
    child_env: &nested_cargo::NestedCargoEnv,
) -> Result<nested_cargo::CapturedOutput> {
    let fixture_dir = fixture_dir();
    let program = locate_or_build_cargo_bdd_command()?;
    let mut cmd = nested_cargo::filtered_command(
        std::path::Path::new(program.get_program()),
        child_env,
        &fixture_dir,
    );
    cmd.arg("steps");
    let output =
        nested_cargo::run_bounded_with_timeout(&mut cmd, nested_cargo::PREPARATION_COMMAND_TIMEOUT)
            .wrap_err("running `cargo bdd steps` for fixture preparation")?;
    Ok(output.into())
}

/// Capture the preparation summary, failing on a non-zero child status.
pub(super) fn capture_preparation(
    child_env: &nested_cargo::NestedCargoEnv,
) -> std::io::Result<nested_cargo::PreparationSummary> {
    let captured = capture_cargo_bdd_steps(child_env)
        .map_err(|error| std::io::Error::other(format!("{error:#}")))?;
    if !captured.status {
        return Err(std::io::Error::other(format!(
            "`cargo bdd steps` failed during fixture preparation:\n{}",
            captured.combined()
        )));
    }
    Ok(record_built(&captured))
}

/// Record that the gate admitted a stamp, so this run compiled nothing.
///
/// A gate that admitted a stamp ran no preparation, so its count is zero by
/// construction — and it must still say so. A report only on the built path
/// would make "nothing was written" and "nothing was compiled" the same
/// observation, which is exactly the confusion a rebuild check has to exclude.
pub(super) fn record_reuse() { record(0); }

/// Record the compilations the timed `cargo bdd steps` itself paid for.
///
/// [`record_reuse`] reports the gate, which is a different fact: it proves no
/// *preparation* ran. It cannot show that the timed command reused the tree,
/// because that command runs later, in the test body. The only evidence for
/// the timed run is its own captured stderr, which the crate's helper returns
/// and its `run_cargo_bdd` caller discards on success — so a check reading it
/// must happen here, while the text is still in hand, rather than by a later
/// scan of output nothing writes.
///
/// Recording the count unconditionally keeps the two observations distinct: a
/// zero here means the timed command compiled nothing, which is the property
/// the workflow gate asserts, and a nonzero value is a rebuild that no scan of
/// the run's output could have seen.
pub(super) fn record_timed(compiling: usize) { record(compiling); }

/// Record the nested build this preparation paid for.
fn record_built(captured: &nested_cargo::CapturedOutput) -> nested_cargo::PreparationSummary {
    let compiled_units = nested_cargo::count_compiling(captured.stderr.as_bytes())
        + nested_cargo::count_compiling(captured.stdout.as_bytes());
    record(compiled_units);
    nested_cargo::PreparationSummary { compiled_units }
}

/// Append this run's nested-build count to the file the caller named.
///
/// Appending rather than truncating is what lets one file hold a whole
/// workflow: each step's count becomes its own line, in step order, so a reader
/// can attribute a rebuild to the step that paid for it. A write failure is
/// swallowed: this is evidence for a separate checker, and a test that failed
/// because a diagnostic could not be filed would be reporting the wrong
/// problem.
fn record(compiled_units: usize) {
    use std::io::Write as _;

    let Some(path) = std::env::var_os(REPORT_PATH) else {
        return;
    };
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    let _ = writeln!(file, "{NESTED_BUILD_MARKER} compiling={compiled_units}");
}
