"""Reading `.config/nextest.toml` as the runner reads it.

Separated from ``timeout_budgets`` so the configuration reading and the
watchdog arithmetic stay legible apart, and so neither module outgrows
the 400-line limit ``AGENTS.md`` sets.

The configuration is parsed with ``tomllib`` rather than matched as
text. A text match finds a key inside a comment, inside a ``filter``
string, or in a table nextest never consults, and reports a budget the
runner does not use.
"""

import tomllib

from timeout_budgets import (
    NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS,
    TERMINATION_SAFETY_MARGIN_SECONDS,
    MissingDefaultProfileError,
    MissingGlobalTimeoutError,
    MissingSlowTimeoutError,
    UnboundedTestError,
    UnparsableConfigurationError,
    seconds,
)


def _parsed(config_text: str) -> dict[str, object]:
    """Return the configuration as TOML.

    Parsed rather than matched as text. A text match finds a key inside
    a comment, inside a ``filter`` string, or in a table nextest never
    consults, and reports a budget the runner does not use. The
    commented-out ``global-timeout`` is the case that matters most here,
    because the contract requires that tier to be present.

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

    Each profile's own table and each of its ``[[overrides]]`` entries,
    with the dotted path naming it so a failure can say which is at
    fault.

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
    """Return every ``slow-timeout`` table the configuration declares.

    Parameters
    ----------
    config_text : str
        A nextest configuration file's text.

    Returns
    -------
    list of tuple
        The dotted path of the declaring table and the ``slow-timeout``
        it holds.
    """
    return [
        (path, _table(table["slow-timeout"]))
        for path, table in _budget_tables(config_text)
        if isinstance(table.get("slow-timeout"), dict)
    ]


def global_timeout(config_text: str) -> float:
    r"""Return the default profile's ``global-timeout`` in seconds.

    Read from ``[profile.default]``'s own table. nextest's other
    profiles inherit it unless they override it, and an ``[[overrides]]``
    entry cannot carry one, so a value found elsewhere is not the budget
    in force.

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
    override leaves every unmatched test running with no bound at all
    while :func:`largest_slow_timeout` still reports a comfortable
    number, so the two are read apart.

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
        and ``terminate-after``.
    """
    own = _table(_table(_parsed(config_text).get("profile")).get(profile))
    table = _table(own.get("slow-timeout"))
    return (
        isinstance(table.get("period"), str)
        and table.get("terminate-after") is not None
    )


def largest_slow_timeout(config_text: str) -> float:
    r"""Return the longest single-test allowance in seconds.

    nextest warns once per ``period`` and terminates after
    ``terminate-after`` of them, so the budget is their product. Every
    multiplier in this repository is one, so a reading that ignored it
    would agree with a correct one against the real file and be wrong
    the moment somebody raised one.

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
    UnboundedTestError
        If a ``slow-timeout`` names no ``terminate-after``, so nextest
        never stops the test it reports as slow.

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
        period = table.get("period")
        if not isinstance(period, str):
            continue
        if table.get("terminate-after") is None:
            raise UnboundedTestError(path)
        budgets.append(seconds(period) * float(str(table["terminate-after"])))
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
    >>> termination_allowance(
    ...     '[profile.default]\n'
    ...     'slow-timeout = { period = "60s", terminate-after = 1, '
    ...     'grace-period = "5s" }\n'
    ... )
    65.0
    """
    periods = [
        seconds(grace)
        for _, table in _slow_timeouts(config_text)
        if isinstance(grace := table.get("grace-period"), str)
    ]
    largest = max(periods, default=NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS)
    return largest + TERMINATION_SAFETY_MARGIN_SECONDS
