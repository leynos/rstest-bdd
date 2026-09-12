"""Reading `.config/nextest.toml` as the runner reads it.

Separated from ``timeout_budgets`` so the configuration reading and the
watchdog arithmetic stay legible apart, and so neither module outgrows
the 400-line limit ``AGENTS.md`` sets. The configuration is parsed with
``tomllib`` rather than matched as text: a text match finds a key inside
a comment, inside a ``filter`` string, or in a table nextest never
consults, and reports a budget the runner does not use.
"""

import tomllib

from timeout_budgets import (
    NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS,
    TERMINATION_SAFETY_MARGIN_SECONDS,
    MalformedGracePeriodError,
    MalformedSlowTimeoutPeriodError,
    MalformedTerminateAfterError,
    MissingDefaultProfileError,
    MissingGlobalTimeoutError,
    MissingSlowTimeoutError,
    UnboundedTestError,
    UnparsableConfigurationError,
    seconds,
)


def _parsed(config_text: str) -> dict[str, object]:
    """Return the configuration as TOML.

    Parsed rather than matched as text, for the reasons the module
    docstring gives. A commented-out ``global-timeout`` is the case that
    matters most here, because the contract requires that tier present.

    Parameters
    ----------
    config_text : str
        A nextest configuration file's text.

    Returns
    -------
    dict[str, object]
        The parsed document.

    Raises
    ------
    UnparsableConfigurationError
        If the text is not valid TOML.
    """
    try:
        return tomllib.loads(config_text)
    except tomllib.TOMLDecodeError as error:
        raise UnparsableConfigurationError(str(error)) from error


def _table(value: object) -> dict[str, object]:
    """Return a parsed value as a table, or an empty one.

    Parameters
    ----------
    value : object
        Any value ``tomllib`` produced.

    Returns
    -------
    dict[str, object]
        The table, or an empty one when the value is not a table.
    """
    return dict(value) if isinstance(value, dict) else {}


def _budget_tables(config_text: str) -> list[tuple[str, dict[str, object]]]:
    """Return every table nextest reads a per-test budget from.

    Each profile's own table and each of its ``[[overrides]]`` entries, with
    the dotted path naming it so a failure can say which is at fault.

    Parameters
    ----------
    config_text : str
        A nextest configuration file's text.

    Returns
    -------
    list of tuple
        The dotted path and the table, in file order.
    """
    tables: list[tuple[str, dict[str, object]]] = []
    for name, raw in _table(_parsed(config_text).get("profile")).items():
        profile = _table(raw)
        tables.append((f"profile.{name}", profile))
        overrides = profile.get("overrides")
        entries = overrides if isinstance(overrides, list) else []
        tables.extend(
            (f"profile.{name}.overrides[{index}]", _table(entry))
            for index, entry in enumerate(entries)
        )
    return tables


def _slow_timeouts(config_text: str) -> list[tuple[str, dict[str, object]]]:
    """Return every ``slow-timeout`` the configuration declares.

    Both of nextest's spellings are read. The table form is used as
    written; the scalar shorthand is folded into ``{ period = ... }``
    before it is returned, and a present value that is neither is refused
    rather than skipped: nextest loads no such file, and skipping one
    leaves the readers reporting another entry's budget.

    Parameters
    ----------
    config_text : str
        A nextest configuration file's text.

    Returns
    -------
    list of tuple
        The dotted path of the declaring table and the ``slow-timeout``
        it holds, with neither spelling lost.

    Raises
    ------
    MalformedSlowTimeoutPeriodError
        If a declared value is neither a non-empty string nor a table.
    """
    declared: list[tuple[str, dict[str, object]]] = []
    for path, table in _budget_tables(config_text):
        match table.get("slow-timeout"):
            case None | "":
                # Absent, and the empty string nextest reads as absent.
                continue
            case str() as period:
                declared.append((path, _table({"period": period})))
            case dict() as written:
                declared.append((path, _table(written)))
            case value:
                raise MalformedSlowTimeoutPeriodError(path, value)
    return declared


def _period(table: dict[str, object], where: str) -> str:
    """Return a ``slow-timeout``'s ``period``, refusing one nextest would.

    nextest requires the key in the table form, so a table without one is
    a configuration error rather than an entry to skip: skipping it would
    report a missing tier while the file names one the runner will not
    load.

    Parameters
    ----------
    table : dict[str, object]
        The ``slow-timeout`` table.
    where : str
        The dotted path of the declaring table.

    Returns
    -------
    str
        The period as nextest spells it.

    Raises
    ------
    MalformedSlowTimeoutPeriodError
        If the table names no period, or names one that is not a string.
    """
    value = table.get("period")
    if not isinstance(value, str):
        raise MalformedSlowTimeoutPeriodError(where, value)
    return value


def _terminate_after(table: dict[str, object], where: str) -> int:
    """Return how many periods nextest lets a slow test run for.

    nextest types ``terminate-after`` as a whole number of periods
    greater than zero and refuses anything else. Reading one anyway
    would put a budget on the tier the runner never applies.

    Parameters
    ----------
    table : dict[str, object]
        The ``slow-timeout`` table.
    where : str
        The dotted path of the declaring table.

    Returns
    -------
    int
        The multiplier, always at least one.

    Raises
    ------
    UnboundedTestError
        If the key is absent, which nextest reads as no termination at all.
    MalformedTerminateAfterError
        If the key holds anything but a whole number above zero.
    """
    value = table.get("terminate-after")
    if value is None:
        raise UnboundedTestError(where)
    if isinstance(value, bool) or not isinstance(value, int):
        raise MalformedTerminateAfterError(where, value)
    if value < 1:
        raise MalformedTerminateAfterError(where, value)
    return value


def _grace_period(table: dict[str, object], where: str) -> float:
    """Return a ``slow-timeout``'s ``grace-period`` in seconds.

    Absence is not a fault: nextest defaults the key to ten seconds, and
    that default is the reading. A value that is present and unreadable
    is a fault, because falling back to the default would size the
    termination allowance against a number the file does not set.

    Parameters
    ----------
    table : dict[str, object]
        The ``slow-timeout`` table.
    where : str
        The dotted path of the declaring table.

    Returns
    -------
    float
        The grace period in seconds.

    Raises
    ------
    MalformedGracePeriodError
        If the key is present but is not a duration string.
    """
    value = table.get("grace-period")
    if value is None:
        return NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS
    if not isinstance(value, str):
        raise MalformedGracePeriodError(where, value)
    return seconds(value)


def global_timeout(config_text: str) -> float:
    r"""Return the default profile's ``global-timeout`` in seconds.

    Read from ``[profile.default]``'s own table. nextest's other profiles
    inherit it unless they override it, and an ``[[overrides]]`` entry cannot
    carry one, so a value found elsewhere is not the budget in force.

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
    profiles = _table(_parsed(config_text).get("profile"))
    if "default" not in profiles:
        raise MissingDefaultProfileError
    budget = _table(profiles["default"]).get("global-timeout")
    if not isinstance(budget, str):
        raise MissingGlobalTimeoutError
    return seconds(budget)


def bounds_a_single_test(config_text: str, profile: str = "default") -> bool:
    """Return whether a profile's own table terminates a slow test.

    An override bounds the tests its filter matches; the profile's own
    bounds the rest. A profile whose only ``terminate-after`` sits in an
    override leaves unmatched tests unbounded while
    :func:`largest_slow_timeout` still reports a comfortable number.

    Parameters
    ----------
    config_text : str
        A nextest configuration file's text.
    profile : str
        The profile to read.

    Returns
    -------
    bool
        True when that profile's own ``slow-timeout`` sets both a period
        and a ``terminate-after`` nextest would accept.
    """
    own = _table(_table(_parsed(config_text).get("profile")).get(profile))
    table = _table(own.get("slow-timeout"))
    if not table:
        return False
    _period(table, f"profile.{profile}")
    if table.get("terminate-after") is None:
        return False
    _terminate_after(table, f"profile.{profile}")
    return True


def largest_slow_timeout(config_text: str) -> float:
    r"""Return the longest single-test allowance in seconds.

    nextest warns once per ``period`` and terminates after
    ``terminate-after`` of them, so the budget is their product. Every
    multiplier in this repository is one, so a reading that ignored it
    would agree with a correct one against the real file and be wrong
    the moment somebody raised one.

    The scalar shorthand is read as well, and is unbounded: nextest reads
    ``slow-timeout = "60s"`` as a period with no termination policy,
    exactly as if the table form had left ``terminate-after`` out.

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
    >>> largest_slow_timeout(
    ...     '[profile.default]\n'
    ...     'slow-timeout = { period = "20m", terminate-after = 1 }\n'
    ... )
    1200.0
    """
    budgets: list[float] = []
    for path, table in _slow_timeouts(config_text):
        period = seconds(_period(table, path))
        budgets.append(period * _terminate_after(table, path))
    if not budgets:
        raise MissingSlowTimeoutError
    return max(budgets)


def termination_allowance(config_text: str) -> float:
    r"""Return the time nextest may take to stop the run, in seconds.

    Two terms, added rather than maximized over, because they answer
    different questions. The first is what nextest promises the test:
    on Unix it signals the process group and waits
    ``slow-timeout.grace-period`` before killing it, read from the
    configuration so a profile that raised it raises the requirement
    too, with nextest's own ten-second default when none is named. The
    second is a fixed margin for the teardown and report writing that
    follow.

    A floor over the two would absorb every grace period below the
    margin, so raising this file's five seconds would have demanded
    nothing more of the watchdog above it. A ``grace-period`` nextest
    could not read is refused where it is read rather than guessed at.

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
    >>> termination_allowance(
    ...     '[profile.default]\n'
    ...     'slow-timeout = { period = "60s", terminate-after = 1, '
    ...     'grace-period = "5s" }\n'
    ... )
    65.0
    """
    periods = [
        _grace_period(table, path) for path, table in _slow_timeouts(config_text)
    ]
    largest = max(periods, default=NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS)
    return largest + TERMINATION_SAFETY_MARGIN_SECONDS
