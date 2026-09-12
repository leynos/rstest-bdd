"""Execution harness for the lockfile-refresh push step.

The contracts in :mod:`derived_fixture_lockfiles_test` pin the push step's
text.  Whether a caller-controlled ref name reaches the shell as script text
or as data is a runtime fact, so these helpers resolve a step the way the
runner does and execute it against a recording ``git``.

The helpers raise rather than assert, so the module carries no blanket lint
suppression and a malformed workflow fails the same way whether or not
assertions are enabled.
"""

import os
import re
import shutil
import stat
import subprocess  # ruff: ignore[suspicious-subprocess-import] - runs a script this repository declares.
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

#: The shell the runner starts for a `run:` fragment, resolved to an absolute
#: path so the harness uses the same interpreter the runner does rather than
#: whichever `bash` a contributor's PATH offers first.
BASH: typ.Final[str] = shutil.which("bash") or "/bin/bash"
#: The plain POSIX shell a ``sh -c`` fragment runs under, pinned as a literal
#: so a contract exercises the runner's interpreter rather than whichever
#: ``sh`` a contributor's PATH resolves first.
POSIX_SHELL: typ.Final[str] = "/bin/sh"
#: A ref name that runs a command when a shell parses it rather than quotes
#: it.  Git permits `$`, `(`, `)`, `;` and `/` in a ref name, so a push target
#: built by interpolation is reachable from the pull request supplying the ref.
HOSTILE_HEAD_REF: typ.Final[str] = "issue/$(touch pwned);ref"
#: The file the hostile ref creates only when the shell parses it as syntax.
INJECTION_ARTEFACT: typ.Final[str] = "pwned"
#: The expression the runner substitutes with the pull request's head ref.
HEAD_REF_EXPRESSION: typ.Final[re.Pattern[str]] = re.compile(
    r"\$\{\{\s*github\.event\.pull_request\.head\.ref\s*\}\}"
)
#: The variable the recording ``git`` appends its argument vectors to.
INVOCATION_LOG_VARIABLE: typ.Final[str] = "GIT_INVOCATIONS"
#: The file the recording ``git`` appends its argument vectors to.
INVOCATION_LOG_NAME: typ.Final[str] = "git-invocations.log"


def read_invocation_log(invocation_log: Path) -> list[list[str]]:
    """Return the argument vectors the recording ``git`` appended.

    The log splits on the newline the recorder writes after each vector and
    nowhere else: a hostile ref name may itself carry a NEL (U+0085) or any
    other character Python's :meth:`str.splitlines` treats as a boundary, and
    parsing those as record separators would report a ref the shell delivered
    whole as if the shell had split it.

    Parameters
    ----------
    invocation_log : pathlib.Path
        The log file :func:`write_recording_git` returned.

    Returns
    -------
    list[list[str]]
        One argv per line, with the recorder's process name stripped.
    """
    # The recorder ends every vector with a newline, so the split leaves one
    # empty trailing element; everything before it is a complete record.
    lines = invocation_log.read_bytes().split(b"\n")[:-1]
    return [
        [
            argument.decode("utf-8", "surrogateescape")
            for argument in line.split(b"\0")[1:]
        ]
        for line in lines
    ]


def substituted(expression: re.Pattern[str], value: str, text: str) -> str:
    """Return *text* with every *expression* replaced by *value*.

    The value is inserted literally, so one carrying backslashes or group
    references reaches the script as the runner would write it.

    Returns
    -------
    str
        The rewritten text.
    """
    return expression.sub(lambda _match: value, text)


def resolve_fragment(
    script: str, declared: cabc.Mapping[str, object], head_ref: str
) -> tuple[str, dict[str, str]]:
    """Return the script and environment the runner hands the shell.

    GitHub substitutes every ``${{ ... }}`` in the script text and in the
    environment before the shell starts, so the harness does the same rather
    than inventing a configuration.  That is what lets a contract tell the two
    forms apart: a step that names the ref inline receives the value as *script
    text*, while one that reads it from the environment receives it as data.

    Parameters
    ----------
    script : str
        The step's ``run`` fragment.
    declared : collections.abc.Mapping[str, object]
        The step's ``env`` mapping, or an empty mapping when it declares none.
    head_ref : str
        The ref name to stand in for the one a pull request supplies.

    Returns
    -------
    tuple[str, dict[str, str]]
        The resolved script, and the environment to run it with.
    """
    return (
        substituted(HEAD_REF_EXPRESSION, head_ref, script),
        {
            str(name): substituted(HEAD_REF_EXPRESSION, head_ref, str(value))
            for name, value in declared.items()
        },
    )


def write_recording_git(directory: Path) -> Path:
    """Put a git stand-in on PATH that records each argument vector it gets.

    Parameters
    ----------
    directory : pathlib.Path
        The directory to write the stand-in into, which the caller also
        prepends to PATH.

    Returns
    -------
    pathlib.Path
        The log file the stand-in appends one NUL-separated argv per line to.
    """
    invocation_log = directory / INVOCATION_LOG_NAME
    recording_git = directory / "git"
    recording_git.write_text(
        "\n".join([
            "#!/usr/bin/env sh",
            "",
            'for argument in "$@"; do',
            f'    printf "\\0%s" "$argument" >> "${INVOCATION_LOG_VARIABLE}"',
            "done",
            f'printf "\\n" >> "${INVOCATION_LOG_VARIABLE}"',
            "",
        ]),
        encoding="utf-8",
    )
    recording_git.chmod(recording_git.stat().st_mode | stat.S_IXUSR)
    return invocation_log


def run_with_recording_git(
    argv: cabc.Sequence[str],
    environment: cabc.Mapping[str, str],
    working_dir: Path,
) -> subprocess.CompletedProcess[str]:
    """Run *argv* with the recording git first on PATH.

    Parameters
    ----------
    argv : collections.abc.Sequence[str]
        The command vector to execute, whose head resolves the shell the
        fragment runs under.
    environment : collections.abc.Mapping[str, str]
        The resolved environment the runner would set for the step.
    working_dir : pathlib.Path
        The directory to run in, which also holds the recording git.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The finished fragment, with its output captured.
    """
    invocation_log = write_recording_git(working_dir)
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - the script is this repository's own.
        argv,
        check=False,
        capture_output=True,
        text=True,
        env={
            **os.environ,
            **environment,
            "PATH": f"{working_dir}{os.pathsep}{os.environ['PATH']}",
            INVOCATION_LOG_VARIABLE: str(invocation_log),
        },
        cwd=working_dir,
        timeout=30,
    )


def run_fragment(
    script: str, environment: cabc.Mapping[str, str], working_dir: Path
) -> subprocess.CompletedProcess[str]:
    """Run *script* as ``bash <file>`` with the recording git first on PATH.

    The fragment goes to a file and runs as ``bash <file>``, which is the form
    the runner uses; a ``bash -c`` harness would execute something the runner
    never does.  The log the recording git writes is
    ``working_dir``/:data:`INVOCATION_LOG_NAME`.

    Parameters
    ----------
    script : str
        The resolved script text.
    environment : collections.abc.Mapping[str, str]
        The resolved environment the runner would set for the step.
    working_dir : pathlib.Path
        The directory to run in, which also holds the recording git.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The finished fragment, with its output captured.
    """
    script_path = working_dir / "push.sh"
    script_path.write_text(script, encoding="utf-8")
    return run_with_recording_git([BASH, str(script_path)], environment, working_dir)


def run_fragment_through_posix_shell(
    script: str, environment: cabc.Mapping[str, str], working_dir: Path
) -> subprocess.CompletedProcess[str]:
    """Run *script* through ``sh -c`` with the recording git first on PATH.

    :func:`run_fragment` runs a fragment the way the runner does, as
    ``bash <file>``.  This helper instead hands the fragment to
    ``/bin/sh -c``, so a property test can attack the push step's parsing
    with generated hostile refs under the plain POSIX shell rather than
    trusting only the runner's bash form.

    Parameters
    ----------
    script : str
        The resolved script text.
    environment : collections.abc.Mapping[str, str]
        The resolved environment the runner would set for the step.
    working_dir : pathlib.Path
        The directory to run in, which also holds the recording git.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The finished fragment, with its output captured.
    """
    return run_with_recording_git([POSIX_SHELL, "-c", script], environment, working_dir)
