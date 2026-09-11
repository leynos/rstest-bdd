"""What the nextest reading counts as configuration, and what it does not.

The contract compares budgets read out of `.config/nextest.toml`, and
those readings can be wrong while the file is right. This repository's
own file cannot expose most of the ways: nothing in it is commented out,
no `filter` names a timeout key, and every `terminate-after` is one, so
a reading that ignored the multiplier would agree with a correct one.

The reading parses the file with `tomllib`. A text match finds a key
inside a comment, inside a `filter` string, or in a table nextest never
consults. The commented-out `global-timeout` is the case that matters
most, because the contract requires that tier to be present.
"""

import typing as typ

import nextest_config as reading
import pytest

if typ.TYPE_CHECKING:
    import collections.abc as cabc

_DEFAULT_PROFILE = (
    "[profile.default]\n"
    'slow-timeout = { period = "60s", terminate-after = 1, '
    'grace-period = "5s" }\n'
    'global-timeout = "75m"\n'
)


def test_a_commented_out_entry_is_not_configuration() -> None:
    """A comment is not configuration, and TOML is what says so.

    The reading parses the file, so a key inside a comment is not read
    as a budget. The commented-out ``global-timeout`` is the case that
    matters most: the contract requires that tier to be present, so a
    text match would have gone on reporting a budget somebody had
    switched off, and the four-tier contract would have passed with
    three.
    """
    config_text = (
        "[profile.default]\n"
        '# global-timeout = "30m"\n'
        '# slow-timeout = { period = "40m", terminate-after = 1, '
        'grace-period = "30m" }\n'
        'slow-timeout = { period = "60s", terminate-after = 1, '
        'grace-period = "5s" }\n'
        'global-timeout = "75m"\n'
    )
    assert reading.global_timeout(config_text) == pytest.approx(4500.0), (
        "a commented-out global-timeout was read as the budget in force"
    )
    assert reading.largest_slow_timeout(config_text) == pytest.approx(60.0), (
        "a commented-out slow-timeout was read as a live one"
    )
    assert reading.termination_allowance(config_text) == pytest.approx(65.0), (
        "a commented-out grace period was read as the one in force"
    )


def test_a_filter_naming_a_timeout_key_is_not_a_budget() -> None:
    """An override's ``filter`` is a string, not configuration.

    A binary named after one of these keys would be matched by a text
    search and read as a budget nextest never applies.
    """
    config_text = (
        "[profile.default]\n"
        'slow-timeout = { period = "60s", terminate-after = 1, '
        'grace-period = "5s" }\n'
        'global-timeout = "75m"\n'
        "\n[[profile.default.overrides]]\n"
        "filter = 'binary(global_timeout_probe) | binary(grace_period_probe)'\n"
        'slow-timeout = { period = "180s", terminate-after = 1 }\n'
    )
    assert reading.largest_slow_timeout(config_text) == pytest.approx(180.0), (
        "the override's own slow-timeout is the largest, not its filter's text"
    )
    assert reading.global_timeout(config_text) == pytest.approx(4500.0), (
        "the filter naming global_timeout_probe was read as a whole-run budget"
    )
    assert reading.termination_allowance(config_text) == pytest.approx(70.0), (
        "the filter naming grace_period_probe was read as a grace period; the "
        "ten seconds here is the override's own table naming none, which "
        "nextest defaults rather than inheriting the profile's five"
    )


@pytest.mark.parametrize(
    "config_text",
    [
        pytest.param(
            '[profile.default]\nslow-timeout = { period = "60s" }\n',
            id="a-profile-without-terminate-after",
        ),
        pytest.param(
            "[profile.default]\n"
            'slow-timeout = { period = "60s", grace-period = "5s" }\n',
            id="a-table-with-only-a-grace-period",
        ),
        pytest.param(
            "[profile.default]\n"
            'slow-timeout = { period = "60s", terminate-after = 1 }\n'
            "\n[[profile.default.overrides]]\n"
            "filter = 'binary(slow)'\n"
            'slow-timeout = { period = "20m" }\n',
            id="an-override-without-terminate-after",
        ),
    ],
)
def test_a_slow_timeout_that_terminates_nothing_is_refused(config_text: str) -> None:
    """`terminate-after` is optional, and without it nothing is bounded.

    nextest treats its absence as no termination: the test is reported
    slow, once per period, and runs on. Counting it as a single period
    would put a number on the tier that is missing, so every comparison
    above it would pass against a budget nextest never applies.

    Every table in `.config/nextest.toml` sets it, so no value here
    changes; this is what stops one appearing.
    """
    with pytest.raises(reading.UnboundedTestError, match=r"terminate-after"):
        reading.largest_slow_timeout(config_text)


def test_a_scalar_slow_timeout_is_read_as_the_unbounded_form() -> None:
    """The scalar shorthand sets a period and nothing else.

    ``slow-timeout = "60s"`` sets ``period`` and leaves
    ``terminate-after`` unset, so the test is reported slow once per
    period and runs on, exactly as the table form does without the key.
    Skipping the scalar would leave the tier invisible, and the largest
    budget reading as whatever the table forms beside it happened to
    say.
    """
    config_text = '[profile.default]\nslow-timeout = "60s"\n'
    with pytest.raises(reading.UnboundedTestError, match=r"terminate-after"):
        reading.largest_slow_timeout(config_text)


def test_a_scalar_slow_timeout_set_to_nothing_is_not_a_budget() -> None:
    """An empty string leaves the key unset rather than zero-length.

    nextest's deserializer returns nothing for an empty string, so the
    profile is left with no per-test budget at all. Reading it as one
    would fail a configuration nextest loads.
    """
    with pytest.raises(reading.MissingSlowTimeoutError):
        reading.largest_slow_timeout('[profile.default]\nslow-timeout = ""\n')


@pytest.mark.parametrize(
    "reads",
    [
        pytest.param(reading.global_timeout, id="global-timeout"),
        pytest.param(reading.largest_slow_timeout, id="largest-slow-timeout"),
        pytest.param(reading.termination_allowance, id="termination-allowance"),
        pytest.param(reading.bounds_a_single_test, id="bounds-a-single-test"),
    ],
)
def test_every_reading_reports_malformed_toml_as_a_configuration_fault(
    reads: cabc.Callable[[str], object],
) -> None:
    """`tomllib` raises `TOMLDecodeError`, a bare `ValueError`.

    Every reading parses the file, so every reading has to report a file
    nextest cannot parse as the configuration fault it is rather than as
    an unhandled parser error several frames below the caller. One
    reader plus the shared parser path would leave the next reader free
    to bypass `_parsed` and grow the same leak again.
    """
    with pytest.raises(reading.UnparsableConfigurationError, match=r"not valid TOML"):
        reads("[profile.default\n")


@pytest.mark.parametrize(
    "config_text",
    [
        pytest.param(
            "[profile.default]\n"
            'slow-timeout = { period = "60s", terminate-after = 0 }\n',
            id="zero-periods",
        ),
        pytest.param(
            "[profile.default]\n"
            'slow-timeout = { period = "60s", terminate-after = -1 }\n',
            id="a-negative-number",
        ),
        pytest.param(
            "[profile.default]\n"
            'slow-timeout = { period = "60s", terminate-after = 1.5 }\n',
            id="a-fraction",
        ),
        pytest.param(
            "[profile.default]\n"
            'slow-timeout = { period = "60s", terminate-after = "2" }\n',
            id="a-string",
        ),
        pytest.param(
            "[profile.default]\n"
            'slow-timeout = { period = "60s", terminate-after = true }\n',
            id="a-boolean",
        ),
    ],
)
def test_a_terminate_after_nextest_would_refuse_is_a_shape_error(
    config_text: str,
) -> None:
    """The key is typed as a whole number of periods above zero.

    Reading any of these as a number would put a budget on the tier the
    runner never applies, and the string form raised a bare `ValueError`
    from the conversion rather than a shape error naming the file. Every
    fault here is a `WorkflowShapeError`, so a malformed configuration
    fails the same way whether or not assertions are enabled.
    """
    with pytest.raises(reading.MalformedTerminateAfterError, match=r"terminate-after"):
        reading.largest_slow_timeout(config_text)


@pytest.mark.parametrize(
    "config_text",
    [
        pytest.param(
            "[profile.default]\nslow-timeout = { terminate-after = 1 }\n",
            id="no-period-at-all",
        ),
        pytest.param(
            "[profile.default]\nslow-timeout = { period = 60, terminate-after = 1 }\n",
            id="a-bare-number",
        ),
    ],
)
def test_a_period_nextest_could_not_read_is_a_shape_error(config_text: str) -> None:
    """The table form requires a ``period`` nextest can read.

    Skipping the entry instead would leave the file naming a tier the
    runner will not load, and the reading would report it as absent
    rather than as the configuration error it is.
    """
    with pytest.raises(reading.MalformedSlowTimeoutPeriodError, match=r"no period"):
        reading.largest_slow_timeout(config_text)


@pytest.mark.parametrize(
    "config_text",
    [
        pytest.param(
            "[profile.default]\n"
            'slow-timeout = { period = "60s", terminate-after = 1, '
            "grace-period = 5 }\n",
            id="a-bare-number",
        ),
        pytest.param(
            "[profile.default]\n"
            'slow-timeout = { period = "60s", terminate-after = 1, '
            "grace-period = true }\n",
            id="a-boolean",
        ),
    ],
)
def test_a_grace_period_nextest_could_not_read_is_a_shape_error(
    config_text: str,
) -> None:
    """The key is typed as a duration string.

    nextest defaults it when it is absent, so absence is not a fault. A
    value that is present and unreadable is: falling back to the default
    would size the termination allowance against a number the file does
    not set, and the watchdog above it against a run nextest will not
    start.
    """
    with pytest.raises(reading.MalformedGracePeriodError, match=r"grace-period"):
        reading.termination_allowance(config_text)


def test_the_per_test_budget_is_a_product_not_a_period() -> None:
    """`terminate-after` scales the period; the budget is their product.

    Every multiplier in `.config/nextest.toml` is one, so a reading that
    ignored the multiplier would agree with a correct one against the
    real file and be wrong the moment somebody raised one. The reading
    took the period alone before this change.
    """
    config_text = (
        '[profile.default]\nslow-timeout = { period = "60s", terminate-after = 5 }\n'
    )
    assert reading.largest_slow_timeout(config_text) == pytest.approx(300.0), (
        "five warning periods of sixty seconds is a 300s budget"
    )


def test_only_the_profile_s_own_table_bounds_an_unmatched_test() -> None:
    """An override bounds what its filter matches, and nothing else.

    A profile whose only `terminate-after` sits in an override leaves
    every test the override does not match with no bound at all, while
    the largest budget still reads comfortable. The two readings are
    separate so the contract can say which is missing.
    """
    only_in_override = (
        "[profile.default]\n"
        'slow-timeout = { period = "60s" }\n'
        "\n[[profile.default.overrides]]\n"
        "filter = 'binary(slow)'\n"
        'slow-timeout = { period = "20m", terminate-after = 1 }\n'
    )
    assert not reading.bounds_a_single_test(only_in_override), (
        "a profile whose only terminate-after is an override's bounds no unmatched test"
    )
    assert reading.bounds_a_single_test(_DEFAULT_PROFILE), (
        "this repository's own default profile bounds a test no override matches"
    )
