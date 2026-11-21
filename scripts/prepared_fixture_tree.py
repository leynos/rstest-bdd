"""Termination of a prepared-fixture step's process tree.

A step runs `make`, which runs `cargo`, which runs `rustc`. When a step
outlives its bound or the driver is interrupted, every one of those has to be
gone before the driver removes the target directory they write to. This module
is that cleanup: it signals the step's process group, confirms the group has
emptied, and says so plainly when it cannot.

It is separate from ``prepared_fixture_step`` so that module stays about
starting a step and reading its report, and so the rules for signalling a
group — which are about process identity, not about workflows — are in one
place.
"""

import os
import signal
import subprocess  # ruff: ignore[suspicious-subprocess-import] - waits on and signals a process it is handed; spawns nothing.
import time
import typing as typ

if typ.TYPE_CHECKING:
    import contextlib

#: How long a terminated tree is given to exit before it is killed outright.
TREE_REAP_SECONDS = 10.0

#: How often the group is asked whether it has emptied.
GROUP_POLL_SECONDS = 0.05


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
    is safe here only because ``run_step`` starts each step in a group of its
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
    stranger. For the same reason the group of a process that has already
    been reaped is never signalled: nothing then ties its id to this step. It
    is only asked whether it still exists, and a group that does is reported
    as a tree that survived. An interrupt reaches here in that state, because
    `Popen.communicate` reaps an exited process before it re-raises.

    Raises
    ------
    TreeSurvivedError
        When the step's process cannot be reaped, or its group still has a
        member, after `SIGKILL` and a further grace period; and when the
        process was already reaped and its group still has a member.
    """
    group = process.pid
    if process.returncode is not None:
        if not group_exited(group):
            msg = (
                f"process {group} was already reaped and its group still has a "
                "member; the group cannot be signalled once its id may be reused"
            )
            raise TreeSurvivedError(msg)
        return
    # An unreaped leader keeps the group alive, so neither call can miss.
    os.killpg(group, signal.SIGTERM)
    await_leader_exit(process.pid)
    os.killpg(group, signal.SIGKILL)
    try:
        reap(process)
    except subprocess.TimeoutExpired as error:
        msg = f"process {process.pid} did not exit after SIGKILL"
        raise TreeSurvivedError(msg) from error
    if not group_exited(group):
        msg = f"process group {group} still has a member after SIGKILL"
        raise TreeSurvivedError(msg)


def reap(process: subprocess.Popen[str]) -> None:
    """Reap *process*, which has been sent `SIGKILL`, within the grace period.

    A function of its own so the one wait this module makes has a name: a
    test of a process that cannot be reaped replaces this, and so leaves
    every other wait in the interpreter alone.

    Notes
    -----
    `Popen.wait` raises `subprocess.TimeoutExpired` when the process has not
    exited within ``TREE_REAP_SECONDS``, and that is passed on to the caller.
    """
    process.wait(timeout=TREE_REAP_SECONDS)


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
