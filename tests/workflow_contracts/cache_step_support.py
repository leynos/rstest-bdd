"""Cache-step anatomy shared by the cache and placement contracts.

:mod:`runner_cache_test`, :mod:`runner_placement_test` and
:mod:`workflow_queries` each read how a workflow declares its caches, and no
two of them may see a different answer. The approved cache action, the single
pinned ref every lane shares, the predicates that recognize a cache step, and
the guard that keeps a suite-running step out of the cache accounting live
here, apart from the document loaders in :mod:`workflow_support`, so that
neither module outgrows the 400-line budget the lint gate enforces.

Every helper answers a question about a step the caller has already parsed, so
the only failure they produce is the unreadable cache declaration that
:func:`cache_paths` raises, drawn from the
:class:`workflow_support.WorkflowShapeError` family like every other shape
violation in these contracts.
"""

import re
from pathlib import PurePosixPath, PureWindowsPath

from workflow_support import CacheStepInputsError, CacheStepPathsError

# Ubicloud's transparent cache intercepts actions/cache v6.1.0 on Linux and
# GitHub serves it on Windows, verified against the Ubicloud cache listing on
# 2026-09-03. One action and one pin therefore serve every lane.
CACHE_ACTION_PREFIX = "actions/cache/"
CACHE_ACTION_REF = "@55cc8345863c7cc4c66a329aec7e433d2d1c52a9"
# Commands that execute the Rust workspace suite. `make test-workflow-contracts`
# is deliberately excluded: it runs this Python suite, not the workspace.
WORKSPACE_TEST_COMMANDS = ("cargo test", "cargo nextest", "make test")


def is_cache_step(step: dict[str, object]) -> bool:
    """Report whether a step invokes one of the approved cache actions.

    Parameters
    ----------
    step : dict[str, object]
        One workflow step.

    Returns
    -------
    bool
        True when the step uses an approved cache action.
    """
    return str(step.get("uses", "")).startswith(CACHE_ACTION_PREFIX)


def cache_paths(step: dict[str, object]) -> list[str]:
    """Return the normalized cache paths a cache step owns.

    Parameters
    ----------
    step : dict[str, object]
        A cache step, as identified by :func:`is_cache_step`.

    Returns
    -------
    list[str]
        One entry per declared path, stripped of surrounding whitespace.

    Raises
    ------
    CacheStepInputsError
        If the step declares no inputs.
    CacheStepPathsError
        If the step declares no path.
    """
    inputs = step.get("with")
    if not isinstance(inputs, dict):
        raise CacheStepInputsError
    raw_path = inputs.get("path")
    if not isinstance(raw_path, str):
        raise CacheStepPathsError
    return [line.strip() for line in raw_path.splitlines() if line.strip()]


def path_components(path: str) -> list[str]:
    """Return every component of a cache path, under either separator.

    A workflow path may use POSIX or Windows separators, and a component such
    as ``target`` can sit at any depth, so both forms are parsed and merged.

    Parameters
    ----------
    path : str
        One declared cache path, possibly containing an expression.

    Returns
    -------
    list[str]
        Every path component under both separator conventions.
    """
    # Expressions such as ${{ github.workspace }} contain no separator of
    # interest, so they survive as a single component either way.
    posix = PurePosixPath(path).parts
    windows = PureWindowsPath(path).parts
    return [part.strip("/\\") for part in (*posix, *windows) if part.strip("/\\")]


def cache_owner(step: dict[str, object]) -> str:
    """Return the logical owner name of a cache step.

    Restore and save steps for the same paths share an owner, and so do the
    Linux and Windows variants of one owner: their ``runner.os`` guards make
    them mutually exclusive.

    Parameters
    ----------
    step : dict[str, object]
        A cache step, as identified by :func:`is_cache_step`.

    Returns
    -------
    str
        The step name without its action verb or its runner-provider suffix.
    """
    name = str(step.get("name", ""))
    name = re.sub(r"^(Restore|Save) ", "", name)
    return re.sub(r"\s*\([^)]*\)$", "", name)


def runs_workspace_tests(step: dict[str, object]) -> bool:
    """Report whether a step runs the Rust workspace suite directly.

    Parameters
    ----------
    step : dict[str, object]
        One workflow step.

    Returns
    -------
    bool
        True when the step's script invokes a workspace test driver.
    """
    script = str(step.get("run", ""))
    for line in script.splitlines():
        command = line.strip()
        for driver in WORKSPACE_TEST_COMMANDS:
            if command == driver or command.startswith(f"{driver} "):
                return True
    return False
