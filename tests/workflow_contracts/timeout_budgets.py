"""Arithmetic behind the four-tier timeout contract.

The contract in :mod:`timeout_ordering_test` compares budgets written down
in three different files. Turning those files into comparable seconds is
the part that can be wrong without any file being wrong, so it lives here
where controlled configurations can drive it.

This repository's own configuration reaches only one of the outcomes these
helpers can produce: every ``grace-period`` in it is five seconds, so the
termination allowance always lands on its floor. A contract that sees only
the floor cannot tell the corrected rule from the one it replaced.

Scope and re-use: these helpers own the reading of nextest duration strings
and the watchdog rule they feed. They serve the workflow contracts under
``tests/workflow_contracts`` and nothing else. They take text rather than
paths, so the caller stays in charge of what it is asserting about;
anything that needs a workflow document should use :mod:`workflow_support`
instead. New timeout tiers belong here beside the three that exist rather
than inline in a contract module.

The helpers raise subclasses of :class:`WorkflowShapeError`, as
:mod:`workflow_support` does, so a malformed configuration fails the same
way whether or not assertions are enabled.
"""

import re
import typing as typ

from workflow_support import WorkflowShapeError

#: What nextest allows a test between ``SIGTERM`` and ``SIGKILL`` when
#: the configuration names no grace period of its own.
NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS: typ.Final[float] = 10.0

#: Added to that grace period to cover the teardown and report writing
#: that follow it. A separate term rather than a floor over the two: a
#: floor absorbs every grace period below it, so raising this file's
#: five seconds to thirty would demand nothing more of the watchdog
#: above it, and the saving would look free until the run it cancelled.
TERMINATION_SAFETY_MARGIN_SECONDS: typ.Final[float] = 60.0

#: ``30s``, ``5m``, ``20 m``: the durations nextest accepts here.
_DURATION: typ.Final[re.Pattern[str]] = re.compile(
    r"^\s*(?P<value>\d+(?:\.\d+)?)\s*(?P<unit>ms|s|m|h)\s*$"
)

_UNIT_SECONDS: typ.Final[dict[str, float]] = {
    "ms": 0.001,
    "s": 1.0,
    "m": 60.0,
    "h": 3600.0,
}

#: ``period`` as its own key. The lookbehind is what keeps
#: ``grace-period`` out: the two keys sit in the same inline table and a
#: substring match would read a termination allowance as a per-test budget.
_PERIOD: typ.Final[re.Pattern[str]] = re.compile(r'(?<![\w-])period\s*=\s*"([^"]+)"')

_GRACE_PERIOD: typ.Final[re.Pattern[str]] = re.compile(r'grace-period\s*=\s*"([^"]+)"')


class UnrecognizedDurationError(WorkflowShapeError):
    """A duration string was not one nextest would accept.

    Raised by :func:`seconds` rather than guessing a magnitude. A
    duration nobody can read is a configuration error, and putting an
    arbitrary number into a budget comparison would hide it behind an
    ordering assertion that then passes or fails for the wrong reason.

    Parameters
    ----------
    duration : str
        The string that could not be read, quoted into the message so
        the failure names the value rather than only the file.

    See Also
    --------
    seconds : The conversion that raises this.
    """

    def __init__(self, duration: str) -> None:
        super().__init__(f"unrecognized nextest duration {duration!r}")


class MissingDefaultProfileError(WorkflowShapeError):
    """The nextest configuration declared no ``[profile.default]``.

    Raised by :func:`global_timeout`. The whole-run budget has to be
    matched to the profile it belongs to, and the default profile is the
    one the coverage lane runs under, so a file without it leaves the
    ordering contract nothing to compare against rather than something
    to compare loosely.

    Takes no parameters: the file is fixed and naming it in the message
    is enough to locate the fault.

    See Also
    --------
    global_timeout : The reading that raises this.
    """

    def __init__(self) -> None:
        super().__init__(
            "nextest.toml must declare a [profile.default] section; the "
            "ordering contract has nothing to compare against without one"
        )


class MissingGlobalTimeoutError(WorkflowShapeError):
    """The default profile set no ``global-timeout``.

    Raised by :func:`global_timeout` when the section exists but the key
    does not. Without it the whole run is unbounded, so the cargo
    watchdog becomes the only limit and a hung run is reported against
    ``cargo`` rather than against the test that hung.

    Takes no parameters, for the same reason as
    :class:`MissingDefaultProfileError`.

    See Also
    --------
    global_timeout : The reading that raises this.
    """

    def __init__(self) -> None:
        super().__init__(
            "[profile.default] must set global-timeout; without it the "
            "whole-run budget is unbounded and the watchdog becomes the "
            "only limit"
        )


class MissingSlowTimeoutError(WorkflowShapeError):
    """The configuration set no per-test budget anywhere.

    Raised by :func:`largest_slow_timeout`. Nothing bounds a single test
    without one, so this is a configuration error rather than a default
    to fall back on: the tier-one comparison would otherwise be made
    against a number nobody chose.

    Takes no parameters, for the same reason as
    :class:`MissingDefaultProfileError`.

    See Also
    --------
    largest_slow_timeout : The reading that raises this.
    """

    def __init__(self) -> None:
        super().__init__("nextest.toml must set at least one slow-timeout period")


def seconds(duration: str) -> float:
    """Convert a nextest duration to seconds.

    Parameters
    ----------
    duration : str
        A duration as nextest spells it, such as ``"75m"``.

    Returns
    -------
    float
        The duration in seconds.

    Raises
    ------
    UnrecognizedDurationError
        If the string is not a duration nextest would accept.

    Examples
    --------
    >>> seconds("75m")
    4500.0
    """
    match = _DURATION.match(duration)
    if match is None:
        raise UnrecognizedDurationError(duration)
    return float(match["value"]) * _UNIT_SECONDS[match["unit"]]


def global_timeout(config_text: str) -> float:
    r"""Return the default profile's ``global-timeout`` in seconds.

    Read textually rather than through a TOML parser, because the value
    must be matched to the profile it belongs to and the file declares
    more than one profile.

    Parameters
    ----------
    config_text : str
        A nextest configuration file's text.

    Returns
    -------
    float
        The default profile's whole-run budget, in seconds.

    Raises
    ------
    MissingDefaultProfileError
        If no ``[profile.default]`` section is present.
    MissingGlobalTimeoutError
        If that section sets no ``global-timeout``.

    Examples
    --------
    >>> global_timeout('[profile.default]\nglobal-timeout = "75m"\n')
    4500.0
    """
    blocks = re.split(r"^\[profile\.", config_text, flags=re.MULTILINE)
    default = next((block for block in blocks if block.startswith("default]")), None)
    if default is None:
        raise MissingDefaultProfileError
    match = re.search(r'^global-timeout\s*=\s*"([^"]+)"', default, re.MULTILINE)
    if match is None:
        raise MissingGlobalTimeoutError
    return seconds(match[1])


def largest_slow_timeout(config_text: str) -> float:
    """Return the longest single-test allowance in seconds.

    Parameters
    ----------
    config_text : str
        A nextest configuration file's text.

    Returns
    -------
    float
        The longest per-test budget.

    Raises
    ------
    MissingSlowTimeoutError
        If the configuration declares no per-test budget.

    Examples
    --------
    >>> largest_slow_timeout('slow-timeout = { period = "20m" }')
    1200.0
    """
    periods = _PERIOD.findall(config_text)
    if not periods:
        raise MissingSlowTimeoutError
    return max(seconds(period) for period in periods)


def termination_allowance(config_text: str) -> float:
    """Return the time nextest may take to stop the run, in seconds.

    Hitting the global timeout starts nextest's ordinary termination
    procedure rather than stopping the run: on Unix it signals the process
    group and waits ``slow-timeout.grace-period`` before killing it; on
    Windows termination is immediate and the grace period is ignored for
    timeouts.

    Two terms, added rather than maximized over, because they answer
    different questions. The first is what nextest promises the test:
    on Unix it signals the process group and waits
    ``slow-timeout.grace-period`` before killing it, read from the
    configuration so a profile that raised it raises the requirement
    too, with nextest's own ten-second default when none is named. The
    second is a fixed margin for the teardown and report writing that
    follow.

    A single floor over the two, which is what this read before, absorbs
    every grace period below the margin. Raising this file's five
    seconds to thirty would have demanded nothing more of the watchdog
    above it, and the saving would have looked free until the run it
    cancelled.

    Parameters
    ----------
    config_text : str
        A nextest configuration file's text.

    Returns
    -------
    float
        The largest configured grace period, or nextest's default, plus
        the safety margin.

    Examples
    --------
    >>> termination_allowance('grace-period = "5s"')
    65.0
    >>> termination_allowance('grace-period = "3m"')
    240.0
    """
    periods = _GRACE_PERIOD.findall(config_text)
    largest = max(
        (seconds(period) for period in periods),
        default=NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS,
    )
    return largest + TERMINATION_SAFETY_MARGIN_SECONDS


def watchdog_requirement(
    global_timeout_seconds: float,
    termination_allowance_seconds: float,
    cold_build_seconds: float,
) -> float:
    """Return the smallest watchdog budget that does not pre-empt nextest.

    The corrected rule, three terms rather than two: the whole-run budget,
    the time nextest takes to terminate a run that spends it, and the
    build that runs inside the watchdog's window but before nextest starts
    its own clock.

    Parameters
    ----------
    global_timeout_seconds : float
        Nextest's whole-run budget.
    termination_allowance_seconds : float
        Time nextest may take to stop the run once that budget is spent.
    cold_build_seconds : float
        Build time inside the ``cargo`` invocation, before nextest starts.

    Returns
    -------
    float
        The smallest acceptable watchdog budget, in seconds.

    Examples
    --------
    >>> watchdog_requirement(4500.0, 60.0, 900.0)
    5460.0
    """
    return global_timeout_seconds + termination_allowance_seconds + cold_build_seconds


def watchdog_shortfall(watchdog_seconds: float, required_seconds: float) -> float:
    """Return how far a watchdog budget falls short of the requirement.

    Zero when the budget is adequate, so a caller can report every
    inadequate lane rather than stopping at the first.

    Parameters
    ----------
    watchdog_seconds : float
        The budget a coverage step declares.
    required_seconds : float
        The budget :func:`watchdog_requirement` calls for.

    Returns
    -------
    float
        The seconds by which the budget is short, or zero.

    Examples
    --------
    >>> watchdog_shortfall(5400.0, 5460.0)
    60.0
    >>> watchdog_shortfall(6600.0, 5460.0)
    0.0
    """
    return max(required_seconds - watchdog_seconds, 0.0)
