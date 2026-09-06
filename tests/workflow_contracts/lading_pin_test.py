"""Contract tests for the lading pin and its compiler-cache reporting.

lading runs the publish dry run, and it is pinned twice: `ci.yml` sets
`LADING_REF` for the job, and the Makefile sets it again so a local
`make publish-check` resolves the same tool. Two pins for one tool drift,
and the failure is quiet: CI validates publish readiness with one version
while a developer validates it with another, and each believes the other
agrees.

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
WORKFLOW_PATH: typ.Final[Path] = REPOSITORY_ROOT / ".github" / "workflows" / "ci.yml"

#: The step that runs the publish dry run.
DRY_RUN_STEP: typ.Final[str] = "Publish dry run"

#: The environment variable lading reads for its statistics file.
STATS_VARIABLE: typ.Final[str] = "LADING_SCCACHE_STATS_JSON"

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


def test_the_two_lading_pins_agree() -> None:
    """CI and the Makefile must resolve the same lading.

    Drift here is quiet rather than loud: both sides keep working, and
    each validates publish readiness against a different tool while
    believing the other agrees.
    """
    workflow_ref = _build_test_env().get("LADING_REF")
    makefile_ref = _makefile_lading_ref()

    assert workflow_ref == makefile_ref, (
        f"ci.yml pins lading at {workflow_ref!r} and the Makefile at "
        f"{makefile_ref!r}; a bump must move both"
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


def test_the_statistics_file_is_uploaded() -> None:
    """A file written into RUNNER_TEMP and never uploaded is not evidence."""
    workflow = _workflow()
    stats_paths = {
        str((step.get("env") or {}).get(STATS_VARIABLE))
        for job in workflow["jobs"].values()
        for step in job.get("steps", [])
        if (step.get("env") or {}).get(STATS_VARIABLE)
    }
    uploads = [
        step
        for job in workflow["jobs"].values()
        for step in job.get("steps", [])
        if "actions/upload-artifact@" in str(step.get("uses", ""))
        and str((step.get("with") or {}).get("path", "")) in stats_paths
    ]

    assert uploads, (
        f"the file named by {STATS_VARIABLE} must be uploaded as an artefact; "
        f"written into the runner's temporary directory and left there, it is "
        f"discarded with the runner"
    )
