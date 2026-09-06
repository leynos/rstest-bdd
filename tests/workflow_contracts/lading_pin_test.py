"""Contract tests for the lading pin and its compiler-cache reporting.

lading runs the publish dry run, and it is pinned three times: `ci.yml`
sets `LADING_REF` for the job, the Makefile sets it again so a local
`make publish-check` resolves the same tool, and `pyproject.toml` pins it
in the `python-tools` group for a bare `uv run lading`. Three pins for one
tool drift, and the failure is quiet: CI validates publish readiness with
one version while a developer validates it with another, and each
believes the other agrees.

The third is the easiest to miss, because the Makefile's `--with` overlay
masks it: `make publish-check` resolves the Makefile's pin regardless of
what the project group holds, so the group can sit generations behind
without any command failing.

The version itself is not asserted. A bump should need one paired change,
not three. What is asserted is that the two agree, and that the workflow
still asks lading for the compiler-cache statistics whose absence made
the publish step's cost unattributable (rstest-bdd#720).

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ
from pathlib import Path

import yaml

REPOSITORY_ROOT: typ.Final[Path] = Path(__file__).resolve().parents[2]
MAKEFILE_PATH: typ.Final[Path] = REPOSITORY_ROOT / "Makefile"
PYPROJECT_PATH: typ.Final[Path] = REPOSITORY_ROOT / "pyproject.toml"
WORKFLOW_PATH: typ.Final[Path] = REPOSITORY_ROOT / ".github" / "workflows" / "ci.yml"

#: The step that runs the publish dry run.
DRY_RUN_STEP: typ.Final[str] = "Publish dry run"

#: The environment variable lading reads for its statistics file.
STATS_VARIABLE: typ.Final[str] = "LADING_SCCACHE_STATS_JSON"

#: The action that collects the statistics file.
UPLOAD_ACTION: typ.Final[str] = "actions/upload-artifact@"

_FULL_SHA: typ.Final[re.Pattern[str]] = re.compile(r"^[0-9a-f]{40}$")


def _makefile_lading_ref() -> str:
    """Return the Makefile's lading pin.

    Returns
    -------
    str
        The commit the Makefile resolves lading from.
    """
    makefile = MAKEFILE_PATH.read_text(encoding="utf-8")
    match = re.search(r"^LADING_REF \?= (?P<ref>\S+)$", makefile, re.MULTILINE)
    assert match is not None, "the Makefile must define LADING_REF"
    return match["ref"]


def _pyproject_lading_ref() -> str:
    """Return the project group's lading pin.

    Returns
    -------
    str
        The commit `uv` resolves lading from for a bare `uv run`.
    """
    pyproject = PYPROJECT_PATH.read_text(encoding="utf-8")
    match = re.search(
        r"lading @ git\+https://github\.com/leynos/lading@(?P<ref>[0-9a-f]{40})",
        pyproject,
    )
    assert match is not None, "pyproject.toml must pin lading by commit"
    return match["ref"]


def _workflow() -> dict[str, typ.Any]:
    """Return the parsed CI workflow.

    Returns
    -------
    dict[str, typ.Any]
        The workflow document.
    """
    return yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))


def _build_test_env() -> dict[str, typ.Any]:
    """Return the build-test job's environment mapping.

    Returns
    -------
    dict[str, typ.Any]
        The job-level environment.
    """
    environment = _workflow()["jobs"]["build-test"].get("env")
    assert isinstance(environment, dict), "build-test must define env"
    return environment


def _dry_run_steps() -> list[dict[str, typ.Any]]:
    """Return every publish dry-run step in the workflow.

    Returns
    -------
    list[dict[str, typ.Any]]
        The steps named for the dry run.
    """
    return [
        step
        for job in _workflow()["jobs"].values()
        for step in job.get("steps", [])
        if step.get("name") == DRY_RUN_STEP
    ]


def test_every_lading_pin_agrees() -> None:
    """All three places must resolve the same lading.

    Drift here is quiet rather than loud: every side keeps working, and
    each validates publish readiness against a different tool while
    believing the others agree. The project group is the easiest to
    forget, because the Makefile's `--with` overlay hides it.
    """
    pins = {
        "ci.yml": _build_test_env().get("LADING_REF"),
        "Makefile": _makefile_lading_ref(),
        "pyproject.toml": _pyproject_lading_ref(),
    }

    assert len(set(pins.values())) == 1, (
        f"every lading pin must name the same commit; a bump must move all "
        f"of them: {pins}"
    )


def test_the_lading_pin_is_a_commit() -> None:
    """A tag or branch would let the tool change under a green pin."""
    makefile_ref = _makefile_lading_ref()

    assert _FULL_SHA.match(makefile_ref), (
        f"LADING_REF must be a full 40-character commit SHA, not a tag or "
        f"branch: {makefile_ref!r}"
    )


def test_the_dry_run_asks_for_compiler_cache_statistics() -> None:
    """The publish step's cost must stay attributable.

    Without this the step is the longest in the job and says nothing
    about what it spent, which is what made the earlier 36-minute runs
    impossible to reason about.
    """
    steps = _dry_run_steps()
    assert steps, f"ci.yml must have a {DRY_RUN_STEP!r} step"

    for step in steps:
        environment = step.get("env") or {}
        assert STATS_VARIABLE in environment, (
            f"the {DRY_RUN_STEP!r} step must set {STATS_VARIABLE} so lading "
            f"reports the compiler cache around each packaged build"
        )
        assert str(environment[STATS_VARIABLE]).endswith(".json"), (
            f"{STATS_VARIABLE} must name a JSON file, got "
            f"{environment[STATS_VARIABLE]!r}"
        )


def _workflow_steps() -> list[dict[str, typ.Any]]:
    """Return every step of every job in the workflow.

    Returns
    -------
    list[dict[str, typ.Any]]
        The steps, in document order.
    """
    return [
        step for job in _workflow()["jobs"].values() for step in job.get("steps", [])
    ]


def _statistics_paths() -> set[str]:
    """Return every path the workflow tells lading to write statistics to.

    Returns
    -------
    set[str]
        The configured file paths.
    """
    return {
        str(value)
        for step in _workflow_steps()
        if (value := (step.get("env") or {}).get(STATS_VARIABLE))
    }


def _uploads_of(paths: set[str]) -> list[dict[str, typ.Any]]:
    """Return the upload steps that collect one of ``paths``.

    Parameters
    ----------
    paths : set[str]
        Paths the workflow writes statistics to.

    Returns
    -------
    list[dict[str, typ.Any]]
        The matching upload steps.
    """
    return [
        step
        for step in _workflow_steps()
        if UPLOAD_ACTION in str(step.get("uses", ""))
        and str((step.get("with") or {}).get("path", "")) in paths
    ]


def test_the_statistics_file_is_uploaded() -> None:
    """A file written into RUNNER_TEMP and never uploaded is not evidence."""
    paths = _statistics_paths()

    assert _uploads_of(paths), (
        f"the file named by {STATS_VARIABLE} must be uploaded as an artefact; "
        f"written into the runner's temporary directory and left there, it is "
        f"discarded with the runner. Configured paths: {sorted(paths)}"
    )
