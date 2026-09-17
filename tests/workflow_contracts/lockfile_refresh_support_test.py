"""Contracts for the lockfile-refresh execution harness.

:mod:`derived_fixture_lockfiles_test` runs a generated example inside a
scratch directory the harness supplies. What that directory has to guarantee
is invisible from the property itself: the property passes just as readily on
a directory another example already wrote into, because the evidence it reads
would still be there. These contracts hold the guarantee instead.

Run with:

    pytest tests/workflow_contracts/lockfile_refresh_support_test.py
"""

import typing as typ

from lockfile_refresh_support import EXAMPLE_DIR_PREFIX, example_working_dir

if typ.TYPE_CHECKING:
    import pytest


def test_each_example_gets_its_own_empty_directory(
    tmp_path_factory: pytest.TempPathFactory,
) -> None:
    """Two examples in one session never share a scratch directory.

    The push-ref property reads an invocation log and looks for an injection
    sentinel by path. A directory carrying either from an earlier example
    makes both readings meaningless, and neither shows up as a failure: a
    stale log reads as a pass, and a stale sentinel reads as a failure
    attributed to the wrong ref.
    """
    first = example_working_dir(tmp_path_factory)
    second = example_working_dir(tmp_path_factory)

    assert first != second, (
        "each generated example must run in its own directory; sharing one "
        "lets an earlier example's invocation log and injection sentinel be "
        f"read as this example's evidence, got {first} twice"
    )
    for directory in (first, second):
        contents = sorted(entry.name for entry in directory.iterdir())
        assert not contents, (
            f"{directory} must be empty when an example starts, or the "
            f"example reads evidence it did not write; found {contents}"
        )


def test_scratch_directories_live_under_the_pytest_base_directory(
    tmp_path_factory: pytest.TempPathFactory,
) -> None:
    """Keep the scratch directories where pytest can find and retire them.

    A directory built from the current working directory, or from the system
    temporary directory directly, escapes pytest's retention policy and
    survives the run. On a shared host that is another agent's disk.
    """
    base = tmp_path_factory.getbasetemp()

    made = (
        example_working_dir(tmp_path_factory),
        example_working_dir(tmp_path_factory),
    )
    for directory in made:
        assert directory.is_relative_to(base), (
            f"{directory} must sit under pytest's base directory {base}, so "
            "the run's scratch is retired with the run"
        )
        assert directory.name.startswith(EXAMPLE_DIR_PREFIX), (
            f"{directory.name!r} must carry the {EXAMPLE_DIR_PREFIX!r} prefix, "
            "which is what names the directory's owner in a failure"
        )
