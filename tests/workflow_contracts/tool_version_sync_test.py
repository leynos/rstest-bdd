"""Contract tests that keep CI and Makefile Python-tool pins in step.

The values are deliberately not asserted here: upgrades should need only a
single paired configuration change. The contract is that CI supplies exactly
the version that the Makefile resolves for every pinned Python tool.

Run via ``make test-workflow-contracts``.
"""

import re
from pathlib import Path

import pytest
import yaml

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
MAKEFILE_PATH = REPOSITORY_ROOT / "Makefile"
WORKFLOW_PATH = REPOSITORY_ROOT / ".github" / "workflows" / "ci.yml"
TOOL_NAMES = ("RUFF", "TY")


def _makefile_tool_version(makefile: str, tool: str) -> str:
    """Return the Makefile version pin for one Python tool."""
    assignment = re.compile(rf"^{tool}_VERSION \?= (?P<version>\S+)$", re.MULTILINE)
    match = assignment.search(makefile)
    assert match is not None, f"the Makefile must define {tool}_VERSION"
    return match["version"]


def _workflow_environment() -> dict[str, str]:
    """Return the build-test environment mapping with a string-only shape."""
    workflow = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    assert isinstance(workflow, dict), "the workflow must be a mapping"
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), "the workflow must define jobs"
    build_test = jobs.get("build-test")
    assert isinstance(build_test, dict), "the workflow must define build-test"
    environment = build_test.get("env")
    assert isinstance(environment, dict), "build-test must define env"
    assert all(
        isinstance(name, str) and isinstance(value, str)
        for name, value in environment.items()
    ), "the workflow environment must contain only string names and values"
    return environment


@pytest.mark.parametrize("tool", TOOL_NAMES)
def test_ci_tool_version_matches_makefile(tool: str) -> None:
    """CI must pass the same version that Make resolves for each tool."""
    environment = _workflow_environment()
    makefile = MAKEFILE_PATH.read_text(encoding="utf-8")
    version_name = f"{tool}_VERSION"
    assert version_name in environment, f"CI must define {version_name}"
    assert environment[version_name] == _makefile_tool_version(makefile, tool), (
        f"CI {tool}_VERSION must match the Makefile {tool}_VERSION pin"
    )
