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

import contextlib
import os
import re
import signal
import subprocess  # ruff: ignore[suspicious-subprocess-import] - runs fixed argvs; no shell.
import time
import typing as typ
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
#:
#: The ``source`` field names which fact the line files, and it is load-bearing
#: rather than decorative: the gate's zero proves no *preparation* ran, while
#: the timed command's count is the only evidence about the timed run itself.
#: The two are written by different code at different points in a run, so a
#: reader that saw only counts would have to assume which question it had an
#: answer to. ``timed`` is the source the reuse judgement requires.
#: ``$`` rather than a bare ``\d+`` matters: without it ``compiling=0oops``
#: matches on the ``0`` prefix and is accepted as a zero-compilation line, so a
#: corrupted report would pass the reuse judgement exactly when the judgement
#: is the only evidence the tree was reused. The writer
#: (``record`` in ``nested_build.rs``) emits this as the whole line, so
#: anchoring at end of line costs nothing a real report depends on.
REPORT_LINE = re.compile(
    rf"{re.escape(BUILD_MARKER)}\s+source=(\w+)\s+compiling=(\d+)$"
)

#: The report source the timed gate files when it admits a stamp.
#:
#: Matches ``ReportSource::Gate`` in
#: ``crates/cargo-bdd/tests/cli/preparation/nested_build.rs``. The reuse
#: judgement requires this source *and* [`TIMED_SOURCE`] in every run it
#: accepts: the gate's line proves the run reached the gate, the timed line
#: proves it reached the command the gate exists to protect.
GATE_SOURCE = "gate"

#: The report source the timed ``cargo bdd steps`` files.
#:
#: Matches ``ReportSource::Timed`` in
#: ``crates/cargo-bdd/tests/cli/preparation/nested_build.rs``.
TIMED_SOURCE = "timed"

#: The sources a passing reuse run must file, exactly once each.
#:
#: A run that files only one of them is not evidence about the other fact. The
#: gate's zero alone is what a run that stopped before the test body leaves
#: behind, and a timed zero alone is what a run whose gate never reported
#: leaves — both were accepted by earlier versions of this check, which is why
#: the pair is required rather than either line on its own.
REQUIRED_SOURCES = (GATE_SOURCE, TIMED_SOURCE)

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

#: How often the group is asked whether it has emptied.
GROUP_POLL_SECONDS = 0.05


class BuildReport(typ.NamedTuple):
    """One line of the nested-build report.

    Attributes
    ----------
    source : str
        Which producer filed the line: ``gate``, ``timed`` or ``preparation``.
        See [`ReportSource`] in
        ``crates/cargo-bdd/tests/cli/preparation/nested_build.rs``.
    compiling : int
        The number of compile units that producer observed, which may be zero.
    """

    source: str
    compiling: int


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


def read_build_reports(target_dir: Path) -> list[BuildReport]:
    """Return the nested-build reports the workflow has recorded so far.

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
    list[BuildReport]
        One report per line, in the order the lines were written, which is the
        order the steps ran. Each carries the source that filed it, because a
        count alone does not say which question it answers.
    """
    try:
        text = build_report_path(target_dir).read_text(encoding="utf-8")
    except FileNotFoundError:
        return []
    reports: list[BuildReport] = []
    for line in text.splitlines():
        match = REPORT_LINE.search(line)
        if match is None:
            continue
        reports.append(
            BuildReport(source=match.group(1), compiling=int(match.group(2)))
        )
    return reports


def timed_test_argv() -> list[str]:
    """Return the argv that runs the timed test through the Makefile."""
    return ["make", RUNNER_TARGET, f"EXPR={TIMED_TEST}"]


def preparation_argv() -> list[str]:
    """Return the argv that runs the documented remedy."""
    return ["make", "prepare-nested-fixtures"]


class TreeSurvivedError(Exception):
    """A timed-out step's process group could not be confirmed gone.

    Raised in place of the timeout, because it changes what the caller may do
    next: a tree that may still be running is a tree that may still be writing
    into the target directory, so that directory must not be removed.
    """


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

    `SIGTERM` goes first, so a process that can flush and exit does, and
    `SIGKILL` follows once the step's own process has exited or the grace
    period has run out. The second signal is not conditional on anything
    being left: `make` exiting says nothing about the compilers beneath it,
    which may ignore the first signal or simply outlive their parent.

    Both signals are sent *before* the step's process is reaped, and that
    order is what protects the group's identity. The group id is the step's
    process id, and an unreaped process keeps its id, so until the reap below
    the id cannot have been handed to an unrelated group. After the reap the
    group is only ever asked whether it still exists, which cannot harm a
    stranger. For the same reason a process the caller has already reaped is
    left alone: nothing then ties its id to this step.

    Raises
    ------
    TreeSurvivedError
        When the step's process cannot be reaped, or its group still has a
        member, after `SIGKILL` and a further grace period.
    """
    if process.returncode is not None:
        return
    group = process.pid
    # An unreaped leader keeps the group alive, so neither call can miss.
    os.killpg(group, signal.SIGTERM)
    await_leader_exit(process.pid)
    os.killpg(group, signal.SIGKILL)
    try:
        process.wait(timeout=TREE_REAP_SECONDS)
    except subprocess.TimeoutExpired as error:
        msg = f"process {process.pid} did not exit after SIGKILL"
        raise TreeSurvivedError(msg) from error
    if not group_exited(group):
        msg = f"process group {group} still has a member after SIGKILL"
        raise TreeSurvivedError(msg)


def await_leader_exit(pid: int) -> None:
    """Wait up to ``TREE_REAP_SECONDS`` for *pid* to exit, without reaping it.

    The exit is observed with ``WNOWAIT``, which leaves the process waitable.
    Reaping it here would release its id while a signal is still to be sent
    to the group that id names.
    """
    deadline = time.monotonic() + TREE_REAP_SECONDS
    flags = os.WEXITED | os.WNOHANG | os.WNOWAIT
    while time.monotonic() < deadline:
        if os.waitid(os.P_PID, pid, flags) is not None:
            return
        time.sleep(GROUP_POLL_SECONDS)


def group_exited(group: int) -> bool:
    """Wait up to ``TREE_REAP_SECONDS`` for every process in *group* to exit.

    Only signal zero is sent, which tests for the group's existence and
    delivers nothing, so this is safe to ask after the leader has been reaped.

    Returns
    -------
    bool
        True once no process is left in the group, False if the wait ran out.
    """
    deadline = time.monotonic() + TREE_REAP_SECONDS
    while time.monotonic() < deadline:
        try:
            os.killpg(group, 0)
        except ProcessLookupError:
            return True
        time.sleep(GROUP_POLL_SECONDS)
    return False


def settle(stack: contextlib.ExitStack, process: subprocess.Popen[str]) -> None:
    """Terminate a timed-out step's tree, abandoning it if it will not go.

    *stack* owns *process*, and leaving it normally waits for the process with
    no bound, which a process that survived `SIGKILL` will not allow. So when
    the tree cannot be confirmed gone the process is taken out of the stack
    and its pipes are closed, and the error reaches the caller.

    Raises
    ------
    TreeSurvivedError
        When [`terminate_tree`] could not confirm the tree gone. The process
        has been abandoned, not reaped, when this is raised.
    """
    try:
        terminate_tree(process)
    except TreeSurvivedError:
        stack.pop_all()
        abandon_pipes(process)
        raise


def abandon_pipes(process: subprocess.Popen[str]) -> None:
    """Close this side of *process*'s pipes without waiting for it.

    Used only for a process that could not be confirmed dead. The driver then
    holds no descriptor on a tree it has given up on; the process is left
    unreaped, because waiting for it is what cannot be bounded.
    """
    for pipe in (process.stdout, process.stderr):
        if pipe is not None:
            pipe.close()


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
        When the step outlives ``STEP_TIMEOUT_SECONDS``.
    KeyboardInterrupt
        When the driver is interrupted while the step runs.

    Notes
    -----
    The tree has been terminated when either is raised. If it could not be
    confirmed gone, [`settle`] raises `TreeSurvivedError` in their place,
    because the caller must then keep the target directory.
    """
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(target_dir)
    env[BUILD_REPORT_ENV] = str(build_report_path(target_dir))
    with contextlib.ExitStack() as stack:
        process = stack.enter_context(
            subprocess.Popen(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed argv, no shell.
                argv,
                cwd=REPO_ROOT,
                env=env,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                start_new_session=True,
            )
        )
        try:
            stdout, stderr = process.communicate(timeout=STEP_TIMEOUT_SECONDS)
        except subprocess.TimeoutExpired, KeyboardInterrupt:
            settle(stack, process)
            raise
        return subprocess.CompletedProcess(argv, process.returncode, stdout, stderr)
