//! Builds and runs the two standalone consumer fixtures under
//! `tests/fixtures/`.
//!
//! `prelude_only` is the load-bearing claim of roadmap 11.2.2: a real crate
//! whose manifest names only `rstest-bdd` and `rstest` writes a complete
//! scenario through `rstest_bdd::prelude` and runs it. Only a separate manifest
//! exercises that, because the derive resolves the runtime path from the
//! *consuming* manifest through `proc_macro_crate`; an in-crate test hits
//! `FoundCrate::Itself` and proves nothing about the case users are in.
//!
//! `macros_direct` is the negative control and R-10's coverage. It names
//! `rstest-bdd-macros` and imports the attributes from there, so the manifest
//! assertion below runs against a case where the crate is genuinely declared.
//! Without it, a reading that silently found nothing would report the prelude
//! fixture as clean for the wrong reason. It also keeps a compiled consumer of
//! the direct import path once no example names that crate any more.
//!
//! Both fixtures are driven with the shared nested-Cargo helpers. Their cold
//! dependency graph is compiled by the `nested_fixture_preparation` test that
//! `.config/nextest.toml` schedules first, so the timed assertions reuse a
//! stamped tree or fail fast naming `make prepare-nested-fixtures`.

use std::io;

use rstest_bdd_harness::nested_cargo;

#[path = "prelude_fixture/preparation.rs"]
mod preparation;

#[path = "prelude_fixture/support.rs"]
mod support;

use support::{MACROS_CRATE, MACROS_DIRECT, PRELUDE_ONLY};

/// Compile both fixture graphs before the timed assertions run.
///
/// The name is the one `.config/nextest.toml` schedules with a preparation
/// budget, and the same one `make prepare-nested-fixtures` selects.
#[test]
fn nested_fixture_preparation() -> io::Result<()> {
    nested_cargo::prepare_now(&preparation::describe()?, preparation::prepare)
}

/// A consumer that names only the facade can write and run a scenario.
#[test]
fn prelude_only_fixture_runs_its_scenario() -> io::Result<()> { run_fixture(PRELUDE_ONLY) }

/// The macro crate's own entry point still works for a consumer that names it.
#[test]
fn macros_direct_fixture_runs_its_scenario() -> io::Result<()> { run_fixture(MACROS_DIRECT) }

/// The V-2 manifest claim: the facade and `rstest`, and nothing else.
///
/// `rstest` is the one entry the prelude cannot hide, because expansion
/// requires it in the consuming manifest. The claim under test is that it is
/// the *only* one, so an exact set comparison is the honest assertion; a
/// `does not contain` check alone would pass on an unread or empty manifest.
#[test]
fn prelude_only_fixture_declares_only_the_facade_and_rstest() -> io::Result<()> {
    let manifest = support::read_manifest(PRELUDE_ONLY)?;
    let declared = support::declared_dependencies(&manifest);
    assert_eq!(
        declared,
        ["rstest", "rstest-bdd"],
        "the V-2 fixture proves nothing unless its manifest names only the facade and rstest",
    );
    Ok(())
}

/// The fixture's `rstest` requirement is the workspace's, not a second literal.
///
/// R-11's mitigation asks the fixture to derive its requirement from
/// `[workspace.dependencies]`. A standalone fixture opts out of the workspace,
/// so `rstest.workspace = true` cannot resolve there; the equality is asserted
/// here instead. A workspace bump that forgets the fixture therefore fails this
/// test rather than leaving two literals to drift.
#[test]
fn prelude_only_fixture_takes_its_rstest_requirement_from_the_workspace() -> io::Result<()> {
    let workspace = support::read_workspace_manifest()?;
    let fixture = support::read_manifest(PRELUDE_ONLY)?;
    let expected = support::workspace_requirement(&workspace, "rstest");
    assert!(
        expected.is_some(),
        "the workspace must state a version requirement for rstest",
    );
    assert_eq!(
        support::dependency_requirement(&fixture, "dev-dependencies", "rstest"),
        expected,
        "the fixture must state the workspace's rstest requirement",
    );
    Ok(())
}

/// The negative control: the same reading finds the macros crate where it is
/// really declared.
#[test]
fn macros_direct_fixture_declares_the_macros_crate() -> io::Result<()> {
    let manifest = support::read_manifest(MACROS_DIRECT)?;
    let declared = support::declared_dependencies(&manifest);
    assert!(
        declared.contains(&MACROS_CRATE),
        "the negative control must declare {MACROS_CRATE}; otherwise the prelude fixture's \
         assertion could not tell a correct manifest from an unread one. Declared: {declared:?}",
    );
    Ok(())
}

/// The prelude fixture's sources reach the prelude and nothing behind it.
///
/// The compiler already rejects a name that is not in the manifest, so this is
/// not a compile check; it pins the fixture's *intent*, so a later edit cannot
/// satisfy the manifest assertion by moving the imports somewhere the manifest
/// test does not read.
#[test]
fn prelude_only_fixture_imports_only_the_prelude() -> io::Result<()> {
    let source = support::read_test_source(PRELUDE_ONLY, "prelude.rs")?;
    assert!(
        source.contains("use rstest_bdd::prelude::*;"),
        "the prelude fixture must glob-import the prelude",
    );
    assert!(
        !source.contains("rstest_bdd_macros"),
        "the prelude fixture must not name the macro crate, directly or in prose",
    );
    Ok(())
}

/// Run one fixture's suite and report a failure with its captured output.
fn run_fixture(fixture: &str) -> io::Result<()> {
    preparation::ensure()?;
    let environment = support::child_environment()?;
    let mut command = support::fixture_command(&environment, fixture, &[]);
    let output = nested_cargo::run_bounded_with_timeout(&mut command, support::RUN_TIMEOUT)?;
    if output.status.success() {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "the {fixture} fixture failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    )))
}
