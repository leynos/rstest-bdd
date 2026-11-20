//! The fixture's dependency closure, measured from the fixture itself.
//!
//! The family declares the packages it builds so its stamp vouches only for
//! those units. A hand-maintained list cannot stay honest at this size, so the
//! closure is read from the fixture's own resolved graph with `cargo tree`.

use std::time::Duration;

use eyre::{Context, Result};
use rstest_bdd_harness::nested_cargo;

use super::fixture_dir;

/// Bound on the graph query when a timed CLI test pays for it.
///
/// `cargo tree` takes the package-cache lock, so it waits rather than fails
/// while another Cargo process holds it. `.config/nextest.toml` gives this
/// binary 180 s, and the query runs before the gate has read the stamp even
/// when the fixture is already prepared, so the preparation budget of nine
/// minutes would always lose: nextest would kill the test first, and the
/// captured child output that names the wait is discarded with it.
///
/// Two minutes rather than the resolution time itself. Measured on the
/// development host against a warm fixture tree, the query took 50 s and then
/// 68 s — both times with no compile to do — because it sat behind the shared
/// package-cache lock; a 60 s bound there was too tight and killed a healthy
/// query. The time is spent waiting for another process, not resolving, so the
/// bound has to cover a queue position as well as the work.
///
/// This figure is also the whole gate's budget, not only the query's. The
/// timed gate creates one [`nested_cargo::Deadline`] from it, and
/// [`cli_preparation_description`](super::cli_preparation_description) charges
/// the query against it before handing back what is left for the two identity
/// probes. So a query that *succeeds* after 119 s leaves the probes a second
/// between them rather than a fresh minute each: a probe with no time left is
/// refused by name, and the test reports that refusal instead of being killed
/// by the runner.
///
/// The stamp read and the witness that follow the probes are local filesystem
/// work and are not charged. A query that spends the whole two minutes reports
/// its own timeout, with the reason, before the runner's 180 s period expires.
///
/// The named preparation test does not use this bound; it has its own period
/// and passes [`nested_cargo::PREPARATION_COMMAND_TIMEOUT`] instead, with
/// nothing else charged against it.
pub(super) const TIMED_GRAPH_QUERY_TIMEOUT: Duration = Duration::from_secs(120);

/// The workspace crates preparing this fixture compiles.
///
/// Preparing this family builds them, so its stamp has to vouch for their
/// units: Cargo rebuilds a dependency whose fingerprint record is gone, and it
/// would do so inside whichever timed assertion ran next. They are outside
/// `FIXTURE_PACKAGE`, so declaring the fixture alone leaves them filtered out
/// of both the recorded and the present name lists, where an evicted record
/// then compares equal to itself and passes the gate.
///
/// The list is the fixture's *whole* dependency closure — not only the two
/// crates its manifest names, and not only its path dependencies. `rstest-bdd`
/// depends on `rstest-bdd-patterns` and `rstest-bdd-policy`, and
/// `rstest-bdd-macros` on those two and `rstest-bdd-harness`, so one build also
/// compiles four workspace crates the manifest never mentions. Beyond those, the
/// build compiles its registry dependencies — `inventory`, `serial_test` and
/// their trees — into the same profile, and a registry crate's units are exactly
/// as evictable as a path crate's even though its sources cannot change. Naming
/// only the path edges left those units filtered out of both lists, so removing
/// one of their fingerprint records compared equal to itself and passed.
///
/// Measured from the fixture's own graph rather than restated here. A
/// handwritten list cannot stay honest at this size: the fixture resolves to
/// well over a hundred registry packages, and the set differs by target triple,
/// so a constant would be both unmaintainable and wrong on at least one lane.
/// `cargo tree` reads the fixture's manifest and lockfile and applies the host's
/// own target filtering, which is the same selection the nested build makes.
///
/// # Errors
///
/// Returns the resolution error when the fixture graph cannot be read, and names
/// the output when the fixture resolves to no package at all. An empty list is
/// not returned: an empty `owned_packages` means "this family claims the whole
/// profile", which is exactly what this family must not do.
///
/// `environment` is the fixture build's, so the query and the build cannot
/// disagree about which graph they resolve.
///
/// `timeout` is the caller's, because the two callers are not in the same
/// position: the named preparation test owns its own period, while a timed
/// assertion has 180 s and already spends part of it on the gate. See
/// [`TIMED_GRAPH_QUERY_TIMEOUT`].
pub(super) fn fixture_dependency_closure(
    environment: &nested_cargo::NestedCargoEnv,
    timeout: Duration,
) -> Result<Vec<String>> {
    let fixture = fixture_dir();
    let mut command = std::process::Command::new(env!("CARGO"));
    // The fixture build's own environment, not this process's. The query has
    // to resolve the graph the nested build resolves, and a `CARGO_HOME`,
    // registry setting or toolchain override the build is given changes that
    // graph without changing anything the test process can see.
    environment.apply_to(&mut command);
    command
        .args(["tree", "--prefix", "none", "--locked", "--offline"])
        .current_dir(&fixture);
    // The Cargo that resolves the graph has to be the one the fixture build
    // runs, and that is `env!("CARGO")` rather than a `PATH` lookup: a host
    // with several toolchains would otherwise measure one Cargo's resolution
    // and hand the result to another's build. Bounded for the same reason the
    // nested builds are — `cargo tree` takes the package-cache lock, so it can
    // wait behind another process rather than fail, and a stall should name
    // itself instead of hanging the test. `--offline` stays: the fixture
    // commits its lockfile, so resolution needs no registry, and a probe that
    // reached one would turn a registry hiccup into a broken-graph report.
    let output = nested_cargo::run_bounded_with_timeout(&mut command, timeout)
        .wrap_err_with(|| format!("resolving the fixture graph in {}", fixture.display()))?;
    let combined = || {
        format!(
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    };
    if !output.status.success() {
        return Err(eyre::eyre!(
            "`cargo tree` failed for the CLI fixture:\n{}",
            combined()
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut names: Vec<String> = text
        .lines()
        // `cargo tree --prefix none` prints one package per line as
        // `<name> v<version>` followed by a path or `(proc-macro)` marker. A
        // continued or repeated entry (`(*)`) still leads with the name, so the
        // first token is the name in every form. Blank lines are separators.
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect();
    names.sort();
    names.dedup();
    if names.is_empty() {
        return Err(eyre::eyre!(
            "the CLI fixture resolved to no package; its output was:\n{}",
            combined()
        ));
    }
    Ok(names)
}
