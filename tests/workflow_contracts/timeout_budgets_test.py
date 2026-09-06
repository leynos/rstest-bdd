"""Unit tests for the four-tier timeout arithmetic.

The ordering contract in :mod:`timeout_ordering_test` reads this
repository's own configuration, where every ``grace-period`` is five
seconds. The termination allowance therefore always lands on its 60 s
floor, and a contract that only ever sees the floor cannot tell the
corrected rule from the one it replaced: deleting the term entirely would
leave the watchdog at 6,600 s and every assertion passing.

These tests drive the derivations with controlled configurations, so each
branch is exercised where the repository's own numbers never reach: a
grace period above the floor, one below it, none at all, and several
profiles disagreeing. The watchdog rule is then checked term by term, and
against a budget sized the old way.

See "Test timeouts: four tiers, outermost last" in
``docs/developers-guide.md``.
"""

import typing as typ

import pytest
import timeout_budgets as budgets

#: A configuration reduced to the keys the derivations read. The real
#: file's prose and overrides say nothing to this arithmetic.
_DEFAULT_PROFILE: typ.Final[str] = """\
[profile.default]
slow-timeout = { period = "60s", terminate-after = 1, grace-period = "5s" }
global-timeout = "75m"
"""


@pytest.mark.parametrize(
    ("config_text", "expected"),
    [
        pytest.param("[profile.default]\n", 60.0, id="no-grace-period-at-all"),
        pytest.param(
            '[profile.default]\ngrace-period = "5s"\n',
            60.0,
            id="grace-period-below-the-floor",
        ),
        pytest.param(
            '[profile.default]\ngrace-period = "60s"\n',
            60.0,
            id="grace-period-at-the-floor",
        ),
        pytest.param(
            '[profile.default]\ngrace-period = "90s"\n',
            90.0,
            id="grace-period-above-the-floor",
        ),
        pytest.param(
            '[profile.default]\ngrace-period = "3m"\n',
            180.0,
            id="grace-period-in-minutes",
        ),
        pytest.param(
            '[profile.default]\ngrace-period = "5s"\n'
            '[profile.long]\ngrace-period = "2m"\n',
            120.0,
            id="largest-of-several-profiles",
        ),
    ],
)
def test_the_termination_allowance_follows_the_configured_grace_period(
    config_text: str, expected: float
) -> None:
    """A raised grace period raises the allowance; the floor catches the rest.

    This is the whole point of reading the value rather than fixing it. A
    profile that gives nextest three minutes to stop the run needs three
    minutes of watchdog to cover it, and a hard-coded 60 s would silently
    stop covering the case it exists for.
    """
    assert budgets.termination_allowance(config_text) == pytest.approx(expected), (
        f"{config_text!r} must yield a {expected:.0f}s termination allowance; a "
        f"watchdog sized from a smaller one would kill the run mid-termination"
    )


def test_the_termination_allowance_ignores_the_per_test_period() -> None:
    """``period`` and ``grace-period`` share an inline table.

    Reading the wrong one would put a twenty-minute per-test budget where
    a five-second termination belongs, restoring by accident the very
    over-sizing this correction removes.
    """
    config_text = '[profile.default]\nslow-timeout = { period = "20m" }\n'
    assert budgets.termination_allowance(config_text) == pytest.approx(60.0), (
        "the termination allowance read a 20m per-test period as a grace "
        "period; only grace-period bounds how long nextest takes to stop"
    )


def test_the_largest_slow_timeout_ignores_the_grace_period() -> None:
    """The per-test budget must not read a grace period either.

    The two keys differ by a prefix, so a substring match returns whichever
    is larger. Here the grace period is deliberately the larger.
    """
    config_text = (
        '[profile.default]\nslow-timeout = { period = "60s", grace-period = "30m" }\n'
    )
    assert budgets.largest_slow_timeout(config_text) == pytest.approx(60.0), (
        "the per-test ceiling read a 30m grace period as a slow-timeout; the "
        "global timeout would then be held to a budget no test can spend"
    )


def test_the_largest_slow_timeout_takes_the_longest_of_several() -> None:
    """Overrides raise the per-test ceiling the global timeout must clear."""
    config_text = _DEFAULT_PROFILE + (
        'slow-timeout = { period = "20m", grace-period = "5s" }\n'
    )
    assert budgets.largest_slow_timeout(config_text) == pytest.approx(20 * 60.0), (
        "the longest override, not the default profile's period, is the "
        "ceiling the global timeout has to clear"
    )


def test_a_configuration_with_no_per_test_budget_is_rejected() -> None:
    """Nothing bounds a single test without one, so this is not a default."""
    with pytest.raises(budgets.MissingSlowTimeoutError):
        budgets.largest_slow_timeout("[profile.default]\n")


def test_the_global_timeout_comes_from_the_default_profile() -> None:
    """Another profile's budget is not the one the coverage lane runs under.

    ``[profile.long]`` sets a shorter global timeout here. Reading the file
    without regard to profile would compare the watchdog against a budget
    no CI job uses.
    """
    config_text = _DEFAULT_PROFILE + '[profile.long]\nglobal-timeout = "30m"\n'
    assert budgets.global_timeout(config_text) == pytest.approx(75 * 60.0), (
        "the whole-run budget must come from [profile.default]; [profile.long] "
        "sets 30m here and no coverage lane runs under it"
    )


def test_a_configuration_with_no_default_profile_is_rejected() -> None:
    """The contract has nothing to compare against without one."""
    with pytest.raises(budgets.MissingDefaultProfileError):
        budgets.global_timeout('[profile.long]\nglobal-timeout = "30m"\n')


def test_a_default_profile_with_no_global_timeout_is_rejected() -> None:
    """An unbounded run leaves the watchdog as the only limit."""
    with pytest.raises(budgets.MissingGlobalTimeoutError):
        budgets.global_timeout("[profile.default]\nslow-timeout = { }\n")


@pytest.mark.parametrize(
    ("duration", "expected"),
    [
        pytest.param("500ms", 0.5, id="milliseconds"),
        pytest.param("5s", 5.0, id="seconds"),
        pytest.param("20 m", 1200.0, id="minutes-with-a-space"),
        pytest.param("1h", 3600.0, id="hours"),
        pytest.param("1.5m", 90.0, id="fractional"),
    ],
)
def test_every_duration_unit_nextest_accepts_converts(
    duration: str, expected: float
) -> None:
    """Each unit is a term in the comparison, so each must convert.

    A unit read as seconds when it means minutes understates a budget by
    sixty-fold, and the contract would pass on a lane that cannot work.
    """
    assert budgets.seconds(duration) == pytest.approx(expected), (
        f"{duration!r} must convert to {expected}s; a misread unit puts an "
        f"order-of-magnitude error straight into a budget comparison"
    )


def test_an_unrecognized_duration_is_rejected_rather_than_guessed() -> None:
    """Guessing would put an arbitrary number into a budget comparison."""
    with pytest.raises(budgets.UnrecognizedDurationError):
        budgets.seconds("soon")


def test_the_watchdog_requirement_is_the_sum_of_its_three_terms() -> None:
    """The corrected rule, stated once, where a test can read it."""
    required = budgets.watchdog_requirement(4500.0, 90.0, 900.0)
    assert required == pytest.approx(5490.0), (
        "the watchdog requirement must be the global timeout plus the "
        "termination allowance plus the cold build"
    )


def test_the_termination_term_is_load_bearing_in_the_requirement() -> None:
    """The correction only means something if the term moves the answer.

    Raising the allowance must raise the requirement one second for one
    second. A rule that ignored the term would return the same number for
    both, which is exactly the state this branch corrects.
    """
    without = budgets.watchdog_requirement(4500.0, 0.0, 900.0)
    with_allowance = budgets.watchdog_requirement(4500.0, 90.0, 900.0)
    assert with_allowance - without == pytest.approx(90.0), (
        f"a 90s allowance moved the requirement by "
        f"{with_allowance - without:.0f}s; the term is not load-bearing"
    )


def test_a_watchdog_sized_without_the_termination_term_falls_short() -> None:
    """The assertion must reject the budget the old rule would have allowed.

    A watchdog covering the whole-run budget and the cold build exactly is
    what a two-term rule calls sufficient. It is short by the termination
    allowance, and the shortfall names that much.
    """
    required = budgets.watchdog_requirement(4500.0, 90.0, 900.0)
    assert budgets.watchdog_shortfall(4500.0 + 900.0, required) == pytest.approx(
        90.0
    ), (
        "a watchdog covering only the global timeout and the cold build must "
        "be reported short by the whole termination allowance"
    )


def test_a_watchdog_at_the_requirement_is_not_short() -> None:
    """The comparison is inclusive; an exactly-sized budget is adequate."""
    required = budgets.watchdog_requirement(4500.0, 90.0, 900.0)
    assert budgets.watchdog_shortfall(required, required) == pytest.approx(0.0), (
        "a budget exactly at the requirement is adequate; reporting it short "
        "would force every lane to carry arbitrary slack"
    )


def test_a_watchdog_above_the_requirement_reports_no_shortfall() -> None:
    """Spare budget is reported as zero, not as a negative deficit.

    The caller prints the shortfall in its failure message, so a negative
    number would read as a lane needing less than it has.
    """
    required = budgets.watchdog_requirement(4500.0, 90.0, 900.0)
    assert budgets.watchdog_shortfall(6600.0, required) == pytest.approx(0.0), (
        "spare budget must report as zero; a negative shortfall would read in "
        "the failure message as a lane needing less than it has"
    )
