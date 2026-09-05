"""Contract for the four timers that can end a test run.

Four independent budgets bound a coverage lane, and each is set in a
different place: a per-test ``slow-timeout`` and a whole-run
``global-timeout`` in ``.config/nextest.toml``, a wall-clock watchdog on
the ``cargo`` invocation in the workflow, and the job's own
``timeout-minutes``. They only work if each sits above the one inside it.

The ordering was inverted for months. The watchdog defaulted to 1,800 s
while nextest was configured for a 75 m run, so a cold compile was killed
by the outer timer before the inner one had spent a third of its budget,
and the failure read as an infrastructure fault rather than as a slow
build. Nothing in ``.config/nextest.toml`` mentions the watchdog, and
nothing in the workflow mentioned nextest, so the two could drift apart
without either side noticing.

These tests read both files and assert the ordering by value. See "Test
timeouts: four tiers, outermost last" in ``docs/developers-guide.md``.
"""

from __future__ import annotations

import re
import typing as typ

import pytest
import yaml

from workflow_support import ROOT

if typ.TYPE_CHECKING:
    from pathlib import Path

NEXTEST_CONFIG = ROOT / ".config" / "nextest.toml"
CI_WORKFLOW = ROOT / ".github" / "workflows" / "ci.yml"

#: The environment variable the shared coverage action reads.
WATCHDOG_VARIABLE = "RUN_RUST_CARGO_WAIT_TIMEOUT"

#: ``30s``, ``5m``, ``20 m``: the durations nextest accepts here.
_DURATION = re.compile(r"^\s*(?P<value>\d+(?:\.\d+)?)\s*(?P<unit>ms|s|m|h)\s*$")

_UNIT_SECONDS = {"ms": 0.001, "s": 1.0, "m": 60.0, "h": 3600.0}


def _seconds(duration: str) -> float:
    """Convert a nextest duration to seconds.

    Parameters
    ----------
    duration : str
        A duration as nextest spells it, such as ``"75m"``.

    Returns
    -------
    float
        The duration in seconds.
    """
    match = _DURATION.match(duration)
    assert match is not None, f"unrecognised nextest duration {duration!r}"
    return float(match["value"]) * _UNIT_SECONDS[match["unit"]]


def _read(path: Path) -> str:
    """Return a file's text."""
    return path.read_text(encoding="utf-8")


def default_profile_global_timeout() -> float:
    """Return the default profile's ``global-timeout`` in seconds.

    Read textually rather than through a TOML parser, because the value
    must be matched to the profile it belongs to and the file declares
    more than one profile.

    Returns
    -------
    float
        The default profile's whole-run budget, in seconds.
    """
    text = _read(NEXTEST_CONFIG)
    section = re.split(r"^\[profile\.", text, flags=re.MULTILINE)
    default = next(
        (block for block in section if block.startswith("default]")),
        None,
    )
    assert default is not None, (
        "nextest.toml must declare a [profile.default] section; the ordering "
        "contract has nothing to compare against without one"
    )
    match = re.search(r'^global-timeout\s*=\s*"([^"]+)"', default, re.MULTILINE)
    assert match is not None, (
        "[profile.default] must set global-timeout; without it the whole-run "
        "budget is unbounded and the watchdog becomes the only limit"
    )
    return _seconds(match[1])


def largest_slow_timeout() -> float:
    """Return the largest per-test ``slow-timeout`` in seconds.

    Returns
    -------
    float
        The longest single-test allowance in the file.
    """
    periods = re.findall(r'period\s*=\s*"([^"]+)"', _read(NEXTEST_CONFIG))
    assert periods, "nextest.toml must set at least one slow-timeout period"
    return max(_seconds(period) for period in periods)


def watchdog_budgets() -> dict[str, float]:
    """Return every cargo-watchdog budget in ``ci.yml``, by step name.

    Returns
    -------
    dict[str, float]
        Step name to budget in seconds.
    """
    document = yaml.safe_load(_read(CI_WORKFLOW))
    budgets: dict[str, float] = {}
    for job_name, job in document["jobs"].items():
        for step in job.get("steps", []):
            value = (step.get("env") or {}).get(WATCHDOG_VARIABLE)
            if value is None:
                continue
            name = str(step.get("name", "")) or job_name
            budgets[name] = float(str(value))
    return budgets


def job_timeout_seconds() -> float:
    """Return the smallest job ``timeout-minutes`` in ``ci.yml``, in seconds.

    Returns
    -------
    float
        The tightest job budget, since that is the one that binds.
    """
    document = yaml.safe_load(_read(CI_WORKFLOW))
    minutes = [
        float(job["timeout-minutes"])
        for job in document["jobs"].values()
        if "timeout-minutes" in job
    ]
    assert minutes, "ci.yml must bound its jobs with timeout-minutes"
    return min(minutes) * 60.0


def test_every_coverage_step_declares_a_watchdog_budget() -> None:
    """The default is invisible, so the value must be written down.

    Leaving the action's 1,800 s default in place is how this went wrong:
    a budget nobody set, mentioned nowhere, killing runs that were merely
    cold.
    """
    budgets = watchdog_budgets()
    assert budgets, (
        f"ci.yml must set {WATCHDOG_VARIABLE} on the steps that run cargo "
        f"under the shared coverage action; relying on the action's default "
        f"leaves the budget undocumented"
    )


@pytest.mark.parametrize("step_name", sorted(watchdog_budgets()))
def test_the_watchdog_sits_above_the_nextest_global_timeout(
    step_name: str,
) -> None:
    """Tier three must not pre-empt tier two.

    A watchdog below the global timeout kills the run before nextest can
    spend the budget it was given, and the error names cargo rather than
    the test that was still going.
    """
    watchdog = watchdog_budgets()[step_name]
    global_timeout = default_profile_global_timeout()

    assert watchdog > global_timeout, (
        f"{step_name} sets {WATCHDOG_VARIABLE}={watchdog:.0f}s, which is not "
        f"above the {global_timeout:.0f}s nextest global-timeout; a cold run "
        f"would be killed by the watchdog before nextest's own budget expired"
    )


def test_the_nextest_global_timeout_sits_above_the_largest_slow_timeout() -> None:
    """Tier two must not pre-empt tier one.

    A global timeout below the longest per-test allowance kills the run
    before the test that allowance exists for can finish.
    """
    global_timeout = default_profile_global_timeout()
    slow_timeout = largest_slow_timeout()

    assert global_timeout > slow_timeout, (
        f"the {global_timeout:.0f}s global-timeout is not above the "
        f"{slow_timeout:.0f}s largest per-test slow-timeout; the run would end "
        f"before that test could use its budget"
    )


@pytest.mark.parametrize("step_name", sorted(watchdog_budgets()))
def test_the_job_timeout_sits_above_the_watchdog(step_name: str) -> None:
    """Tier four must not pre-empt tier three.

    A job timeout at or below the watchdog turns a budget overrun into a
    cancellation, which discards the log that would have explained it.
    """
    watchdog = watchdog_budgets()[step_name]
    job_timeout = job_timeout_seconds()

    assert job_timeout > watchdog, (
        f"the job timeout of {job_timeout:.0f}s is not above "
        f"{step_name}'s {watchdog:.0f}s watchdog; an overrun would be "
        f"cancelled rather than reported"
    )
