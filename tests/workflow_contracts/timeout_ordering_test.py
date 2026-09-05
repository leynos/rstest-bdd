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

Two of the four timers do not start together, which the ordering has to
allow for. The watchdog starts when ``cargo`` starts, and covers the
build as well as the test run; nextest's global timeout starts only once
tests begin. The job timer starts when the job starts, long before
coverage and long before the work that follows it. Comparing the
configured numbers alone would call an inverted lane correct, so the
allowances below are measured rather than assumed.

See "Test timeouts: four tiers, outermost last" in
``docs/developers-guide.md``.
"""

import re
import typing as typ

import pytest
import yaml
from workflow_support import ROOT

if typ.TYPE_CHECKING:
    from pathlib import Path

NEXTEST_CONFIG: typ.Final[Path] = ROOT / ".config" / "nextest.toml"
CI_WORKFLOW: typ.Final[Path] = ROOT / ".github" / "workflows" / "ci.yml"

#: The environment variable the shared coverage action reads.
WATCHDOG_VARIABLE: typ.Final[str] = "RUN_RUST_CARGO_WAIT_TIMEOUT"

#: The action whose steps must declare a watchdog budget.
COVERAGE_ACTION: typ.Final[str] = "shared-actions/.github/actions/generate-coverage"

#: Build time inside the `cargo` invocation, before nextest starts its
#: own clock. The watchdog covers it; the global timeout does not.
#: Measured at 3 m 31 s on run 33966769942 with a nearly cold compiler
#: cache; 15 minutes is that with room to spare.
COLD_BUILD_ALLOWANCE_SECONDS: typ.Final[float] = 15 * 60.0

#: Everything in the job that is not the coverage step. The job timer
#: covers it; the watchdog does not. Measured at 14 m 03 s before and
#: 36 m 41 s after on run 33971821695, the latter almost entirely the
#: publish dry run.
NON_COVERAGE_ALLOWANCE_SECONDS: typ.Final[float] = 55 * 60.0

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
    assert match is not None, f"unrecognized nextest duration {duration!r}"
    return float(match["value"]) * _UNIT_SECONDS[match["unit"]]


@pytest.fixture(scope="module")
def nextest_config() -> str:
    """Return the nextest configuration file's text.

    Returns
    -------
    str
        The file's contents.
    """
    return NEXTEST_CONFIG.read_text(encoding="utf-8")


@pytest.fixture(scope="module")
def ci_workflow() -> dict[str, typ.Any]:
    """Return the parsed CI workflow.

    Returns
    -------
    dict[str, typ.Any]
        The parsed workflow document.
    """
    return yaml.safe_load(CI_WORKFLOW.read_text(encoding="utf-8"))


@pytest.fixture(scope="module")
def global_timeout(nextest_config: str) -> float:
    """Return the default profile's ``global-timeout`` in seconds.

    Read textually rather than through a TOML parser, because the value
    must be matched to the profile it belongs to and the file declares
    more than one profile.

    Parameters
    ----------
    nextest_config : str
        The nextest configuration file's text.

    Returns
    -------
    float
        The default profile's whole-run budget, in seconds.
    """
    blocks = re.split(r"^\[profile\.", nextest_config, flags=re.MULTILINE)
    default = next((block for block in blocks if block.startswith("default]")), None)
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


@pytest.fixture(scope="module")
def largest_slow_timeout(nextest_config: str) -> float:
    """Return the longest single-test allowance in seconds.

    Parameters
    ----------
    nextest_config : str
        The nextest configuration file's text.

    Returns
    -------
    float
        The longest per-test budget.
    """
    periods = re.findall(r'period\s*=\s*"([^"]+)"', nextest_config)
    assert periods, "nextest.toml must set at least one slow-timeout period"
    return max(_seconds(period) for period in periods)


class CoverageLane(typ.NamedTuple):
    """One coverage step, with the job budget that encloses it.

    Attributes
    ----------
    job : str
        The job the step belongs to.
    step : str
        The step's declared name.
    watchdog : float | None
        The step's watchdog budget in seconds, or ``None`` when it sets
        none and so inherits the action's default.
    job_timeout : float | None
        The enclosing job's ``timeout-minutes`` in seconds, or ``None``
        when the job declares none.
    """

    job: str
    step: str
    watchdog: float | None
    job_timeout: float | None

    def __str__(self) -> str:
        """Return a location suitable for a failure message.

        Returns
        -------
        str
            ``job:step`` for this lane.
        """
        return f"{self.job}:{self.step!r}"


def _optional_seconds(value: object) -> float | None:
    """Return a ``timeout-minutes`` value in seconds, or None.

    Parameters
    ----------
    value : object
        The declared value, or ``None`` when the job declares none.

    Returns
    -------
    float | None
        The budget in seconds.
    """
    return None if value is None else float(str(value)) * 60.0


@pytest.fixture(scope="module")
def lanes(ci_workflow: dict[str, typ.Any]) -> tuple[CoverageLane, ...]:
    """Return every coverage lane, each with its own job's ceiling.

    Every coverage step is included, not only those that set the
    watchdog, so a step that loses its override is visible as ``None``
    rather than absent. An absent entry would make the ordering tests
    skip it silently and restore the default that caused the regression.

    The job ceiling travels with the lane rather than being taken as the
    tightest in the file: an unrelated job's budget has nothing to say
    about this one, and comparing them would either fail an
    honestly-sized job or force unrelated budgets to move together.

    Parameters
    ----------
    ci_workflow : dict[str, typ.Any]
        The parsed CI workflow.

    Returns
    -------
    tuple[CoverageLane, ...]
        One entry per coverage step.
    """
    lanes: list[CoverageLane] = []
    for job_name, job in ci_workflow["jobs"].items():
        for step in job.get("steps", []):
            if COVERAGE_ACTION not in str(step.get("uses", "")):
                continue
            raw = (step.get("env") or {}).get(WATCHDOG_VARIABLE)
            lanes.append(
                CoverageLane(
                    job=str(job_name),
                    step=str(step.get("name", "")) or str(job_name),
                    watchdog=None if raw is None else float(str(raw)),
                    job_timeout=_optional_seconds(job.get("timeout-minutes")),
                )
            )
    return tuple(lanes)


def test_every_coverage_step_declares_a_watchdog_budget(
    lanes: tuple[CoverageLane, ...],
) -> None:
    """The default is invisible, so every step must write it down.

    Leaving the action's 1,800 s default in place is how this went
    wrong: a budget nobody set, mentioned nowhere, killing runs that
    were merely cold. Checking that some step sets it would not do;
    one step losing its override is enough to bring the fault back.
    """
    assert lanes, (
        f"ci.yml must invoke {COVERAGE_ACTION}; this contract has nothing to "
        f"assert against otherwise"
    )
    missing = [str(lane) for lane in lanes if lane.watchdog is None]
    assert not missing, (
        f"these coverage steps do not set {WATCHDOG_VARIABLE} and so inherit "
        f"the action's undocumented 1,800 s default: {missing}"
    )


def test_the_watchdog_covers_the_nextest_budget_and_the_build(
    lanes: tuple[CoverageLane, ...],
    global_timeout: float,
    largest_slow_timeout: float,
) -> None:
    """Tier three must not pre-empt tier two.

    The two clocks do not start together. The watchdog starts with
    ``cargo`` and covers the build; nextest's global timeout starts only
    once tests begin. A watchdog merely above the global timeout is
    still pre-empting it whenever the build takes longer than the
    difference, so the build allowance is part of the comparison.

    The far end matters too. A test already running when the global
    timeout expires is allowed to finish, so the run can outlast that
    budget by the longest per-test allowance. Whether that tail is ever
    reached or not, allowing for it costs nothing: the watchdog only
    fires on an overrun.
    """
    required = global_timeout + largest_slow_timeout + COLD_BUILD_ALLOWANCE_SECONDS
    for lane in lanes:
        assert lane.watchdog is not None, str(lane)
        assert lane.watchdog >= required, (
            f"{lane} sets {WATCHDOG_VARIABLE}={lane.watchdog:.0f}s, below the "
            f"{required:.0f}s needed to cover the {global_timeout:.0f}s nextest "
            f"budget, the {largest_slow_timeout:.0f}s a test already running may "
            f"still take, and {COLD_BUILD_ALLOWANCE_SECONDS:.0f}s of cold build; "
            f"a cold run would be killed before nextest's own budget expired"
        )


def test_the_nextest_global_timeout_sits_above_the_largest_slow_timeout(
    global_timeout: float,
    largest_slow_timeout: float,
) -> None:
    """Tier two must not pre-empt tier one.

    A global timeout below the longest per-test allowance kills the run
    before the test that allowance exists for can finish.
    """
    assert global_timeout > largest_slow_timeout, (
        f"the {global_timeout:.0f}s global-timeout is not above the "
        f"{largest_slow_timeout:.0f}s largest per-test slow-timeout; the run "
        f"would end before that test could use its budget"
    )


def test_the_job_timeout_covers_the_watchdog_and_the_rest_of_the_job(
    lanes: tuple[CoverageLane, ...],
) -> None:
    """Tier four must not pre-empt tier three.

    The job timer starts when the job starts, not when coverage does.
    Formatting, linting, the published-GPUI scenario and the publish dry
    run all sit outside the watchdog's window but inside the job's, so a
    job timeout merely above the watchdog still cancels the run before
    the watchdog can report it, and a cancellation discards the log that
    would have explained the overrun.
    """
    for lane in lanes:
        assert lane.watchdog is not None, str(lane)
        assert lane.job_timeout is not None, (
            f"{lane} runs cargo under a {lane.watchdog:.0f}s watchdog in a job "
            f"with no timeout-minutes; the outermost tier is missing"
        )
        required = lane.watchdog + NON_COVERAGE_ALLOWANCE_SECONDS
        assert lane.job_timeout >= required, (
            f"{lane} has a job timeout of {lane.job_timeout:.0f}s, below the "
            f"{required:.0f}s needed to cover its {lane.watchdog:.0f}s watchdog "
            f"plus {NON_COVERAGE_ALLOWANCE_SECONDS:.0f}s of measured work "
            f"outside it; an overrun would be cancelled rather than reported"
        )
