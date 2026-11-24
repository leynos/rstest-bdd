//! Runs a trybuild test body in a child copy of the test binary.
//!
//! `trybuild` and the nested `cargo` invocations read `CARGO_TARGET_DIR` and
//! `RUST_BACKTRACE` from the environment they inherit. A test must not change
//! its own process environment to arrange that, so the parent starts a fresh
//! child of this binary with `Command::env` and `Command::env_remove`, and the
//! child runs the body under that environment.

use std::{
    env,
    io::{self, Write as _},
    process::Command,
};

use camino::Utf8Path;

/// Names the variable whose presence makes this binary run the body itself.
const CHILD_MARKER: &str = "RSTEST_BDD_TRYBUILD_SCOPED_CHILD";

/// Prefix of the fixture filter argument `trybuild` reads from the arguments.
const FILTER_PREFIX: &str = "trybuild=";

/// The libtest name of a test, given its `module_path!()`-qualified name: the
/// binary's own crate segment is not part of the name libtest filters on.
fn libtest_name(qualified: &str) -> &str {
    qualified
        .split_once("::")
        .map_or(qualified, |(_crate, rest)| rest)
}

/// The arguments the child is started with: the single test to run, and every
/// `trybuild=<substring>` fixture filter the parent was given, because
/// `trybuild` and the preparation gate read their filter from the process
/// arguments and a child that dropped it would run the full fixture set.
fn child_arguments(qualified_test: &str, parent_arguments: &[String]) -> Vec<String> {
    let mut arguments = vec![
        "--exact".to_owned(),
        libtest_name(qualified_test).to_owned(),
        "--nocapture".to_owned(),
    ];
    arguments.extend(
        parent_arguments
            .iter()
            .filter(|argument| argument.starts_with(FILTER_PREFIX))
            .cloned(),
    );
    arguments
}

/// Runs `body` with `CARGO_TARGET_DIR` set to `target_directory` and
/// `RUST_BACKTRACE` removed.
///
/// `qualified_test` is the calling test's `concat!(module_path!(), "::", name)`.
/// In the parent this re-executes the current test alone, with the scoped
/// environment, and fails with the child's output when it fails; in the child
/// it just runs `body`. The child's output is buffered, then replayed on
/// success or carried in the error on failure, so a failure keeps its full
/// diagnostics; the accepted cost is that no progress shows until the child
/// ends.
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
    let parent_arguments: Vec<String> = env::args_os()
        .skip(1)
        .filter_map(|argument| argument.into_string().ok())
        .collect();
    let output = Command::new(env::current_exe()?)
        .args(child_arguments(qualified_test, &parent_arguments))
        .env(CHILD_MARKER, "1")
        .env("CARGO_TARGET_DIR", target_directory)
        .env_remove("RUST_BACKTRACE")
        .output()?;
    if output.status.success() {
        // Pass the child's progress output through, as the in-process run showed
        // it under `--nocapture`.
        io::stdout().write_all(&output.stdout)?;
        io::stderr().write_all(&output.stderr)?;
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
    //! Unit tests for the child's name and argument derivation.

    use std::{env, io};

    use camino::Utf8Path;

    use super::{CHILD_MARKER, child_arguments, libtest_name, run_scoped};

    /// A target directory no real run uses, so the parent can tell its own
    /// environment apart from the child's.
    const PROBE_TARGET: &str = "/scoped-child-probe-target";

    /// The child sees `CARGO_TARGET_DIR` set and `RUST_BACKTRACE` removed and
    /// runs the body; the parent's own environment keeps neither change.
    #[test]
    fn run_scoped_gives_the_child_the_scoped_environment() {
        let result = run_scoped(
            concat!(
                module_path!(),
                "::run_scoped_gives_the_child_the_scoped_environment"
            ),
            Utf8Path::new(PROBE_TARGET),
            || {
                let target = env::var("CARGO_TARGET_DIR");
                if target.as_deref() != Ok(PROBE_TARGET) {
                    return Err(io::Error::other(format!(
                        "the child's CARGO_TARGET_DIR was {target:?}, not {PROBE_TARGET}"
                    )));
                }
                if env::var_os("RUST_BACKTRACE").is_some() {
                    return Err(io::Error::other("the child still had RUST_BACKTRACE"));
                }
                Ok(())
            },
        );
        result.expect("the scoped child should run the body and pass");
        if env::var_os(CHILD_MARKER).is_none() {
            assert_ne!(
                env::var("CARGO_TARGET_DIR").as_deref(),
                Ok(PROBE_TARGET),
                "the parent's own environment must not receive the child's CARGO_TARGET_DIR"
            );
        }
    }

    /// A body error in the child fails the parent's call, and the message the
    /// child printed reaches the parent's error.
    #[test]
    fn run_scoped_reports_a_failing_child_body() {
        let result = run_scoped(
            concat!(module_path!(), "::run_scoped_reports_a_failing_child_body"),
            Utf8Path::new(PROBE_TARGET),
            || Err(io::Error::other("probe body failure")),
        );
        if env::var_os(CHILD_MARKER).is_some() {
            // The child fails its own test with the body's error.
            result.expect("child body result");
            return;
        }
        let err = result.expect_err("a failing child body must fail the parent's call");
        assert!(
            err.to_string().contains("probe body failure"),
            "the child's failure message must reach the parent's error: {err}"
        );
    }

    #[test]
    fn the_child_receives_the_trybuild_fixture_filter() {
        let parent = ["--nocapture", "step", "trybuild=ui_lints", "--exact"]
            .map(str::to_owned)
            .to_vec();
        let arguments = child_arguments("trybuild_macros::runner::step_macros_compile", &parent);
        assert_eq!(
            arguments,
            [
                "--exact",
                "runner::step_macros_compile",
                "--nocapture",
                "trybuild=ui_lints"
            ],
            "the child must run the one test and carry the parent's trybuild= filter"
        );
    }

    #[test]
    fn an_unfiltered_parent_gives_the_child_no_filter() {
        let parent = ["--nocapture".to_owned()];
        let arguments = child_arguments("trybuild_macros::runner::step_macros_compile", &parent);
        assert!(
            !arguments
                .iter()
                .any(|argument| argument.starts_with("trybuild=")),
            "an unfiltered parent must not give the child a trybuild= filter: {arguments:?}"
        );
    }

    #[test]
    fn libtest_name_drops_the_crate_segment() {
        assert_eq!(
            libtest_name("trybuild_macros::runner::step_macros_compile"),
            "runner::step_macros_compile",
            "the binary's own crate segment is not part of the libtest name"
        );
        assert_eq!(
            libtest_name("bare"),
            "bare",
            "a name without a crate segment is used as given"
        );
    }
}
