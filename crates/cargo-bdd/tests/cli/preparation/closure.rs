//! The fixture's dependency closure, measured from the fixture itself.
//!
//! The family declares the packages it builds so its stamp vouches only for
//! those units. A hand-maintained list cannot stay honest at this size, so the
//! closure is read from the fixture's own resolved graph with `cargo tree`.

use eyre::{Context, Result};

use super::fixture_dir;

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
pub(super) fn fixture_dependency_closure() -> Result<Vec<String>> {
    let fixture = fixture_dir();
    let output = std::process::Command::new("cargo")
        .args(["tree", "--prefix", "none", "--locked", "--offline"])
        .current_dir(&fixture)
        // `cargo tree` writes the graph to stdout and progress or warnings to
        // stderr; the parse below needs only the former, but a failure has to
        // report both or the cause is invisible.
        .output()
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
