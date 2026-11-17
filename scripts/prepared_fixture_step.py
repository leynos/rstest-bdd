"""Run one step of the prepared-fixture workflow, and bound it.

The driver in ``check_prepared_fixture_workflow`` composes steps and judges
their outcomes; this module is what actually starts a command, hands it the
right environment, and makes the step's time bound mean what it says.

Both halves it owns are easy to get subtly wrong in a way no assertion catches.
The environment has to leave the build flags to the Makefile, so the run half and
the preparation half cannot disagree about ``RUSTFLAGS``. And the timeout has to
reach the step's whole process tree, because killing ``make`` alone leaves the
nested ``cargo`` and ``rustc`` writing into a directory the driver then deletes.

The nested-build report lives here too, because this module is what names its
file in the environment: a reader looking for that channel should find it beside
the code that opens it.
"""

import os
import re
import signal
import subprocess  # ruff: ignore[suspicious-subprocess-import] - runs fixed argvs; no shell.
from pathlib import Path

#: The repository root, two levels above ``scripts/``.
REPO_ROOT = Path(__file__).resolve().parents[1]

#: The marker the CLI fixture's gate prefixes its nested-build count with.
#:
#: It has to match ``NESTED_BUILD_MARKER`` in
#: ``crates/cargo-bdd/tests/cli/preparation/nested_build.rs`` and is duplicated
#: rather than shared because the two languages have no common header. The
#: reader's contract is the format, not the constant's identity.
BUILD_MARKER = "cargo-bdd-nested-build:"

#: Environment variable naming the file the fixture's gate appends to.
#:
#: Matches the constant of the same purpose on the Rust side. A file, because
#: both streams the report could travel on are discarded: the test helper drops
#: the child's capture on success and nextest suppresses a passing test's output.
BUILD_REPORT_ENV = "RSTEST_BDD_NESTED_BUILD_REPORT"

#: The one report line shape this workflow knows how to read.
REPORT_LINE = re.compile(rf"{re.escape(BUILD_MARKER)}\s+compiling=(\d+)")

#: The timed test the CLI family gates on preparation.
#:
#: Chosen because it is cheap, it is a real user-visible command
#: (``cargo bdd steps``), and its binary also holds the family's
#: ``nested_fixture_preparation`` test, so the target and the timed test share
#: one filtered suite.
#:
#: The filter is binary-qualified and uses nextest's *exact* test-name
#: operator. Both parts are load-bearing. A ``test(/.../)`` regex is matched
#: against the *test name*, not the ``<binary>::<test>`` path nextest prints,
#: so a filter written as ``test(/cargo-bdd::cli::list_steps_runs$/)`` matches
#: nothing at all; nextest then exits non-zero with ``no tests to run``, which
#: a driver reading only the exit status would score as the expected refusal,
#: and the whole gate would "pass" while running no test. This is the same
#: empty-filter hazard the Makefile contract test exists for.
#:
#: ``test(=name)`` is also why this string survives Make. Make expands ``$``
#: in a command-line variable, so an anchored ``test(/^list_steps_runs$/)``
#: would reach nextest as ``test(/^list_steps_runs/)`` unless escaped as
#: ``$$``. Exact match needs no anchor and so needs no escaping.
TIMED_TEST = "binary_id(cargo-bdd::cli) & test(=list_steps_runs)"

#: The remedy the refusal must name, so a reader is told what to run.
REMEDY = "make prepare-nested-fixtures"

#: The run half of the same policy, which owns the build environment.
#:
#: A bare ``cargo nextest run`` here would carry the caller's ``RUSTFLAGS``
#: rather than the preparation's, and refuse the stamp the preparation wrote.
RUNNER_TARGET = "nextest-run"

#: Bound each step so a hung nested build fails the gate instead of the job.
STEP_TIMEOUT_SECONDS = 1800

#: How long a terminated tree is given to exit before it is killed outright.
TREE_REAP_SECONDS = 10.0


def build_report_path(target_dir: Path) -> Path:
    """Return the file this workflow's nested-build reports are appended to.

    Inside the private target directory rather than a temporary file of its
    own, so it is removed with the tree it describes and cannot outlive the
    run that produced it.

    Returns
    -------
    Path
        The report file's path below *target_dir*.
    """
    return target_dir / "nested-build-report.txt"


def read_build_reports(target_dir: Path) -> list[int]:
    """Return the nested-build counts the workflow has recorded so far.

    Read from the file the fixture's gate appends to, not from the step's
    captured output. That distinction is the whole point of the file: the
    helper which spawns ``cargo bdd steps`` captures the command's stderr and
    discards it on success, and nextest suppresses a passing test's output, so
    the captured output carries no nested compilation at all. A scan of it for
    ``Compiling`` lines found nothing whatever the build did, and read that
    emptiness as reuse.

    A missing file yields an empty list rather than raising, and the caller is
    expected to treat that as a failure: a workflow whose evidence channel was
    never written cannot distinguish a reused tree from a step that quietly did
    nothing.

    Returns
    -------
    list[int]
        One count per report line, in the order the lines were written, which
        is the order the steps ran.
    """
    try:
        text = build_report_path(target_dir).read_text(encoding="utf-8")
    except FileNotFoundError:
        return []
    counts: list[int] = []
    for line in text.splitlines():
        match = REPORT_LINE.search(line)
        if match is None:
            continue
        counts.append(int(match.group(1)))
    return counts


def timed_test_argv() -> list[str]:
    """Return the argv that runs the timed test through the Makefile."""
    return ["make", RUNNER_TARGET, f"EXPR={TIMED_TEST}"]


def preparation_argv() -> list[str]:
    """Return the argv that runs the documented remedy."""
    return ["make", "prepare-nested-fixtures"]


def terminate_tree(process: subprocess.Popen[str]) -> None:
    """Kill *process* and every descendant before returning.

    `subprocess.run`'s own timeout handling kills only the direct child, which
    here is `make`. Its `cargo` and `rustc` descendants are separate processes
    that inherit the same process group, keep writing into the target
    directory, and outlive the kill; the caller then deletes that directory
    while they are still building in it. The delete appears to succeed, the
    orphaned compilers fail later against a tree that vanished, and the
    workflow reports a timeout whose real damage happened afterwards.

    Signalling the whole group is what makes the bound mean what it says. It
    is safe here only because [`run_step`] starts each step in a group of its
    own, so this signal cannot reach the driver or its parent.

    The escalation is deliberate rather than impatient: `SIGTERM` first, so a
    compiler that can flush and exit does; `SIGKILL` only after a bounded
    wait, because a tree that ignores the polite signal must still be gone
    before the caller removes the directory it is writing to.
    """
    if process.poll() is not None:
        return
    try:
        group = os.getpgid(process.pid)
    except ProcessLookupError:
        return
    for sig in (signal.SIGTERM, signal.SIGKILL):
        try:
            os.killpg(group, sig)
        except ProcessLookupError:
            return
        try:
            process.wait(timeout=TREE_REAP_SECONDS)
        except subprocess.TimeoutExpired:
            continue
        else:
            return


def run_step(
    argv: list[str],
    *,
    target_dir: Path,
) -> subprocess.CompletedProcess[str]:
    """Run one workflow command against *target_dir* and return its result.

    Only ``CARGO_TARGET_DIR`` and the report path are set here. The
    build-relevant environment — ``RUSTFLAGS`` above all — is the Makefile's to
    own: this module and the targets it drives must not diverge on it, which is
    the failure that once made the workflow's third step reproduce its first.

    Each step runs in its own process group so a timeout can be enforced
    against the whole tree; see [`terminate_tree`].

    Parameters
    ----------
    argv : list[str]
        The command and its arguments.
    target_dir : Path
        The private target directory every step shares.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The captured result, never raised on a non-zero status: each step
        decides for itself what an acceptable outcome is.

    Raises
    ------
    subprocess.TimeoutExpired
        When the step outlives ``STEP_TIMEOUT_SECONDS``. The tree has already
        been terminated when this is raised.
    """
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(target_dir)
    env[BUILD_REPORT_ENV] = str(build_report_path(target_dir))
    with (
        subprocess.Popen(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed argv, no shell.
            argv,
            cwd=REPO_ROOT,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            start_new_session=True,
        ) as process,
    ):
        try:
            stdout, stderr = process.communicate(timeout=STEP_TIMEOUT_SECONDS)
        except subprocess.TimeoutExpired:
            terminate_tree(process)
            raise
        return subprocess.CompletedProcess(argv, process.returncode, stdout, stderr)
