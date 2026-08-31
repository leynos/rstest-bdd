"""Contract tests for the active internal Python baseline.

These tests intentionally inspect only live tooling, helper scripts, and
workflow configuration. Historical plans, fixture lockfiles, Rust dependency
versions, and numbered design sections are outside this contract.

Run via ``make test-workflow-contracts``.
"""

import collections.abc as cabc
import re
import tomllib
from pathlib import Path

import pytest
import yaml

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
PYTHON_VERSION = "3.14"
PYTHON_REQUIREMENT = f">={PYTHON_VERSION}"
RUFF_TARGET = "py314"
PEP_723_BLOCK = re.compile(
    r"^# /// script\n(?P<body>.*?)^# ///$",
    re.MULTILINE | re.DOTALL,
)
LOW_PYTHON_VERSION = re.compile(r"(?<!\d)3\.(?:12|13)(?!\d)")
PYTHON_RUNTIME_CONTEXTS = frozenset({"env", "inputs", "matrix", "with"})


def _load_toml(path: Path) -> dict[str, object]:
    """Parse a repository TOML file."""
    with path.open("rb") as source:
        return tomllib.load(source)


def _pep_723_metadata(path: Path) -> dict[str, object] | None:
    """Return PEP 723 metadata when an internal script declares it."""
    source = path.read_text(encoding="utf-8")
    match = PEP_723_BLOCK.search(source)
    if match is None:
        return None
    body = "\n".join(line.removeprefix("# ") for line in match["body"].splitlines())
    return tomllib.loads(body)


def _is_python_version_setting(name: str, path: tuple[str, ...]) -> bool:
    """Return whether a workflow key selects a Python runtime version."""
    if name == "python-version":
        return True
    return name == "python" and not PYTHON_RUNTIME_CONTEXTS.isdisjoint(path)


def _workflow_version_values(
    value: object,
    path: tuple[str, ...] = (),
) -> cabc.Iterator[tuple[str, object]]:
    """Yield configured Python-version values from a workflow tree."""
    match value:
        case cabc.Mapping():
            for key, child in value.items():
                key_name = str(key)
                normalized = key_name.lower().replace("_", "-")
                child_path = (*path, key_name)
                if _is_python_version_setting(normalized, path):
                    yield ".".join(child_path), child
                yield from _workflow_version_values(child, child_path)
        case list():
            for index, child in enumerate(value):
                yield from _workflow_version_values(child, (*path, str(index)))


@pytest.mark.parametrize("filename", ["pyproject.toml", "uv.lock"])
def test_project_and_lock_require_python_314(filename: str) -> None:
    """The project and generated lock data must share the 3.14 floor."""
    configuration = _load_toml(REPOSITORY_ROOT / filename)
    if filename == "pyproject.toml":
        project = configuration.get("project")
        assert isinstance(project, dict), "pyproject.toml must define [project]"
        requirement = project.get("requires-python")
    else:
        requirement = configuration.get("requires-python")
    assert requirement == PYTHON_REQUIREMENT, (
        f"{filename} must require {PYTHON_REQUIREMENT}; got {requirement!r}"
    )


def test_python_analysers_target_python_314() -> None:
    """Ruff, Pylint, Ty, and isolated helper linting must target 3.14."""
    configuration = _load_toml(REPOSITORY_ROOT / "pyproject.toml")
    tool = configuration.get("tool")
    assert isinstance(tool, dict), "pyproject.toml must define [tool]"
    ruff = tool.get("ruff")
    pylint = tool.get("pylint")
    assert isinstance(ruff, dict), "pyproject.toml must configure Ruff"
    assert isinstance(pylint, dict), "pyproject.toml must configure Pylint"
    pylint_main = pylint.get("main")
    assert isinstance(pylint_main, dict), "pyproject.toml must configure Pylint main"
    assert ruff.get("target-version") == RUFF_TARGET, (
        f"Ruff must target {RUFF_TARGET}; got {ruff.get('target-version')!r}"
    )
    assert pylint_main.get("py-version") == PYTHON_VERSION, (
        f"Pylint must target {PYTHON_VERSION}; got {pylint_main.get('py-version')!r}"
    )

    makefile = (REPOSITORY_ROOT / "Makefile").read_text(encoding="utf-8")
    obsolete_target = re.compile(
        r"--(?:python-version|python|target-version) "
        r"(?:3\.(?:12|13)|py31[23])"
    )
    observed = {
        "df12": "DF12_PYTHON ?= 3.14" in makefile,
        "direct-python3": "python3 scripts/" in makefile,
        "obsolete-target": obsolete_target.search(makefile) is not None,
        "project-python": (
            "PROJECT_PYTHON = $(UV_ENV) $(UV) run --python 3.14 python" in makefile
        ),
        "ruff-target-count": makefile.count("--target-version py314"),
        "ty": "$(TY) check --python-version 3.14" in makefile,
    }
    expected = {
        "df12": True,
        "direct-python3": False,
        "obsolete-target": False,
        "project-python": True,
        "ruff-target-count": 2,
        "ty": True,
    }
    assert observed == expected, (
        f"Makefile Python targets must use the 3.14 baseline: {observed!r}"
    )


def test_internal_uv_scripts_require_python_314() -> None:
    """Every active PEP 723 script must require Python 3.14."""
    scripts = sorted((REPOSITORY_ROOT / "scripts").rglob("*.py"))
    metadata = {
        path.relative_to(REPOSITORY_ROOT).as_posix(): script_metadata
        for path in scripts
        if (script_metadata := _pep_723_metadata(path)) is not None
    }
    assert metadata, "at least one internal script must declare PEP 723 metadata"
    requirements = {
        path: script_metadata.get("requires-python")
        for path, script_metadata in metadata.items()
    }
    expected = dict.fromkeys(requirements, PYTHON_REQUIREMENT)
    assert requirements == expected, (
        f"active PEP 723 scripts must require {PYTHON_REQUIREMENT}: {requirements!r}"
    )


@pytest.mark.parametrize(
    "workflow_path",
    sorted((REPOSITORY_ROOT / ".github" / "workflows").glob("*.y*ml")),
    ids=lambda path: path.name,
)
def test_workflows_do_not_configure_old_python(
    workflow_path: Path,
) -> None:
    """Active workflows must not select Python 3.12 or 3.13 runtimes."""
    workflow = yaml.safe_load(workflow_path.read_text(encoding="utf-8"))
    for setting, value in _workflow_version_values(workflow):
        assert LOW_PYTHON_VERSION.search(str(value)) is None, (
            f"{workflow_path.name}:{setting} configures an obsolete Python "
            f"runtime: {value!r}"
        )
