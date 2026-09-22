"""Contracts for the lockfile-refresh execution harness.

:mod:`derived_fixture_lockfiles_test` runs a generated example inside a
scratch directory the harness supplies. What that directory has to guarantee
is invisible from the property itself: the property passes just as readily on
a directory another example already wrote into, because the evidence it reads
would still be there. These contracts hold the guarantee instead.

Run with:

    pytest tests/workflow_contracts/lockfile_refresh_support_test.py
"""

import ast
import typing as typ

from lockfile_refresh_support import EXAMPLE_DIR_PREFIX, example_working_dir
from workflow_support import repository_file

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


#: The property whose settings are held, and where it is declared.
PROPERTY_MODULE = ("tests", "workflow_contracts", "derived_fixture_lockfiles_test.py")
PROPERTY_NAME = "test_push_step_treats_any_generated_head_ref_as_inert_data"


def _property_settings() -> dict[str, ast.expr]:
    """Return the keywords of the push-ref property's `hypothesis.settings`.

    Read from the source rather than from Hypothesis's runtime attributes,
    which are private. The property is found by name, and exactly one
    `settings(...)` decorator is required.

    Returns
    -------
    dict[str, ast.expr]
        Each keyword the decorator passes, mapped to its expression.
    """
    module = ast.parse(repository_file(*PROPERTY_MODULE))
    function = next(
        node
        for node in module.body
        if isinstance(node, ast.FunctionDef) and node.name == PROPERTY_NAME
    )
    calls = [
        decorator
        for decorator in function.decorator_list
        if isinstance(decorator, ast.Call)
        and ast.unparse(decorator.func).endswith("settings")
    ]
    assert len(calls) == 1, f"{PROPERTY_NAME} must declare one settings decorator"
    return {keyword.arg: keyword.value for keyword in calls[0].keywords if keyword.arg}


def test_the_push_ref_property_declares_no_deadline() -> None:
    """Hold the property to `deadline=None`, the fix for the reported flake.

    Every example starts a shell and a recording `git`, so its wall time is a
    property of the host. A finite deadline fails under load while asserting
    nothing about the push step, and reintroducing one would pass every other
    case here on an idle machine.
    """
    deadline = _property_settings().get("deadline")

    assert isinstance(deadline, ast.Constant), (
        f"{PROPERTY_NAME} must declare deadline=None; it declares "
        f"{ast.unparse(deadline) if deadline is not None else 'no deadline'}"
    )
    assert deadline.value is None, (
        f"{PROPERTY_NAME} must declare deadline=None, not {deadline.value!r}"
    )


def test_the_push_ref_property_suppresses_no_health_check() -> None:
    """Refuse the fixture-scope suppression the session-scoped helper retired.

    The property takes `tmp_path_factory`, which is session-scoped, so
    Hypothesis has nothing to warn about. Suppressing
    `function_scoped_fixture` again would only hide a function-scoped
    fixture reappearing, which is the shared-directory defect this helper
    removed.
    """
    suppressed = _property_settings().get("suppress_health_check")

    assert suppressed is None, (
        f"{PROPERTY_NAME} must suppress no health check; it suppresses "
        f"{ast.unparse(suppressed)}"
    )
