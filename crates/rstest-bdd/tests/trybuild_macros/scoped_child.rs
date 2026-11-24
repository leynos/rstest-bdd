//! Runs a trybuild test body in a child copy of the test binary.
//!
//! `trybuild` and the nested `cargo` invocations read `CARGO_TARGET_DIR` and
//! `RUST_BACKTRACE` from the environment they inherit. A test must not change
//! its own process environment to arrange that, so the parent starts a fresh
//! child of this binary with `Command::env` and `Command::env_remove`, and the
//! child runs the body under that environment.

use std::{env, io, process::Command};

use camino::Utf8Path;

/// Names the variable whose presence makes this binary run the body itself.
const CHILD_MARKER: &str = "RSTEST_BDD_TRYBUILD_SCOPED_CHILD";

/// The libtest name of a test, given its `module_path!()`-qualified name: the
/// binary's own crate segment is not part of the name libtest filters on.
fn libtest_name(qualified: &str) -> &str {
    qualified
        .split_once("::")
        .map_or(qualified, |(_crate, rest)| rest)
}

/// Runs `body` with `CARGO_TARGET_DIR` set to `target_directory` and
/// `RUST_BACKTRACE` removed.
///
/// `qualified_test` is the calling test's `concat!(module_path!(), "::", name)`.
/// In the parent this re-executes the current test alone, with the scoped
/// environment, and fails with the child's output when it fails; in the child
/// it just runs `body`.
///
/// # Errors
///
/// Returns the body's own error in the child, and in the parent the spawn error
/// or an error carrying the failed child's status and output.
pub(crate) fn run_scoped(
    qualified_test: &str,
    target_directory: &Utf8Path,
    body: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    if env::var_os(CHILD_MARKER).is_some() {
        return body();
    }
    let output = Command::new(env::current_exe()?)
        .args(["--exact", libtest_name(qualified_test), "--nocapture"])
        .env(CHILD_MARKER, "1")
        .env("CARGO_TARGET_DIR", target_directory)
        .env_remove("RUST_BACKTRACE")
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "{qualified_test} failed in its scoped child ({}):\n{}{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )))
}

#[cfg(test)]
mod tests {
    //! Unit tests for the libtest name derivation.

    use super::libtest_name;

    #[test]
    fn libtest_name_drops_the_crate_segment() {
        assert_eq!(
            libtest_name("trybuild_macros::runner::step_macros_compile"),
            "runner::step_macros_compile"
        );
        assert_eq!(libtest_name("bare"), "bare");
    }
}
