//! Bounded execution of a nested `cargo` invocation.
//!
//! Both output pipes are drained concurrently, because a chatty build would
//! otherwise fill a pipe buffer and deadlock the child on a blocking write.
//! The child is killed when the wall-clock bound expires, and the error
//! carries the command context plus the output captured so far.
//!
//! Draining to EOF is the natural terminator and is wrong on the timeout path:
//! killing the direct child does not reap its grandchildren, they inherit the
//! pipe write ends, and a join waits for *them* rather than for the bound.
//! `Drain::collect_until` therefore caps the post-kill collection instead.

use std::{
    io,
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use super::drain::Drain;

/// Per-invocation wall-clock bound for each nested `cargo` call.
///
/// Realistic warm invocations finish well inside this bound; it exists to
/// turn a hang into a named failure with the captured output attached rather
/// than letting the test runner's own timeout kill the test with a
/// runner-generated message.
pub(super) const CHILD_TIMEOUT: Duration = Duration::from_secs(300);

/// Grace period for collecting output after the child has exited or been
/// killed.
///
/// Output is normally at EOF the moment the child exits, so this only covers
/// descendants that inherited the pipe write ends — `cargo` does not reap a
/// `rustc` it started when it is itself killed. Waiting for those to exit is
/// what let an expired preparation bound run on until the test runner killed
/// the whole test, so the wait is capped instead.
pub(super) const DRAIN_GRACE: Duration = Duration::from_secs(2);

/// Run a child process to completion under the wall-clock bound, draining
/// both output pipes concurrently.
///
/// On timeout the child is killed and the error carries the output captured
/// so far.
///
/// # Errors
///
/// Returns the spawn or wait error when the child cannot be spawned, and a
/// [`std::io::ErrorKind::TimedOut`] error carrying the captured output when
/// the bound is exceeded.
pub fn run_bounded(cmd: &mut Command) -> io::Result<Output> {
    run_bounded_with_timeout(cmd, CHILD_TIMEOUT)
}

/// [`run_bounded`] with an explicit timeout, for callers that size their own
/// bound.
///
/// # Errors
///
/// As [`run_bounded`].
pub fn run_bounded_with_timeout(cmd: &mut Command, timeout: Duration) -> io::Result<Output> {
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let Some(stdout_pipe) = child.stdout.take() else {
        return Err(broken_pipe("stdout"));
    };
    let Some(stderr_pipe) = child.stderr.take() else {
        return Err(broken_pipe("stderr"));
    };
    let mut stdout = Drain::spawn(stdout_pipe);
    let mut stderr = Drain::spawn(stderr_pipe);

    match wait_bounded(&mut child, timeout) {
        Outcome::Exited(status) => Ok(Output {
            status,
            // Measured from the exit, not from the spawn: the child may have
            // used the whole bound, and its output is already buffered in the
            // drainers by then.
            stdout: stdout.collect_until(Instant::now() + DRAIN_GRACE),
            stderr: stderr.collect_until(Instant::now() + DRAIN_GRACE),
        }),
        Outcome::TimedOut => {
            let _ = child.kill();
            let _ = child.wait();
            Err(timeout_error(
                timeout,
                &stdout.collect_until(Instant::now() + DRAIN_GRACE),
                &stderr.collect_until(Instant::now() + DRAIN_GRACE),
            ))
        }
        Outcome::Failed(err) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout.collect_until(Instant::now() + DRAIN_GRACE);
            let _ = stderr.collect_until(Instant::now() + DRAIN_GRACE);
            Err(err)
        }
    }
}

/// Result of the bounded wait.
enum Outcome {
    /// The child exited with `status`.
    Exited(std::process::ExitStatus),
    /// The wall-clock bound elapsed before the child exited.
    TimedOut,
    /// Waiting on the child failed.
    Failed(io::Error),
}

/// Poll the child until it exits, the deadline passes, or waiting fails.
fn wait_bounded(child: &mut Child, timeout: Duration) -> Outcome {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Outcome::Exited(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    return Outcome::TimedOut;
                }
            }
            Err(err) => return Outcome::Failed(err),
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// Error for a missing piped stream; `run_bounded` always pipes both.
fn broken_pipe(stream: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::UnexpectedEof,
        format!("nested cargo did not provide a piped {stream}; run_bounded always pipes"),
    )
}

/// Build the timeout error with the command context and captured output.
fn timeout_error(timeout: Duration, stdout: &[u8], stderr: &[u8]) -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        format!(
            "nested cargo exceeded its {} s wall-clock bound; the child was killed;\nstdout so \
             far:\n{}\nstderr so far:\n{}",
            timeout.as_secs(),
            String::from_utf8_lossy(stdout),
            String::from_utf8_lossy(stderr)
        ),
    )
}
