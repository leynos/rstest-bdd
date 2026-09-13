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
MODULE_LINE_BUDGET = 400
PEP_723_BLOCK = re.compile(
    r"^# /// script\n(?P<body>.*?)^# ///$",
    re.MULTILINE | re.DOTALL,
)
LOW_PYTHON_VERSION = re.compile(r"(?<!\d)3\.(?:1[0-3]|[0-9])(?!\d)")
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
        "pylint-python": "PYLINT_PYTHON ?= 3.14" in makefile,
        "ruff-target-count": makefile.count("--target-version py314"),
        "ty": (
            "$(TY) check --python-version 3.14 "
            "$(PYTHON_TARGETS) $(SPELLING_PY_SRCS)" in makefile
        ),
    }
    expected = {
        "df12": True,
        "direct-python3": False,
        "obsolete-target": False,
        "project-python": True,
        "pylint-python": True,
        "ruff-target-count": 2,
        "ty": True,
    }
    assert observed == expected, (
        f"Makefile Python targets must use the 3.14 baseline: {observed!r}"
    )


def test_pylint_measures_every_module_against_the_line_budget() -> None:
    """Pylint itself enforces the module line budget, on the sources' grammar.

    The budget used to lapse on a module the pass could not parse, because a
    parser that cannot read a file reports nothing for it at all. Running the
    pass on the interpreter the sources are written for removes the cause, and
    reporting syntax errors removes the silence.
    """
    configuration = _load_toml(REPOSITORY_ROOT / "pyproject.toml")
    tool = configuration.get("tool")
    assert isinstance(tool, dict), "pyproject.toml must define [tool]"
    pylint = tool.get("pylint")
    assert isinstance(pylint, dict), "pyproject.toml must configure Pylint"
    pylint_main = pylint.get("main")
    assert isinstance(pylint_main, dict), "pyproject.toml must configure Pylint main"
    messages = pylint.get("messages control")
    assert isinstance(messages, dict), (
        "pyproject.toml must configure Pylint's message control"
    )
    enable = messages.get("enable")
    disable = messages.get("disable")
    assert isinstance(enable, list), "Pylint must declare the messages it enables"
    assert isinstance(disable, list), "Pylint must declare the messages it disables"

    assert pylint_main.get("max-module-lines") == MODULE_LINE_BUDGET, (
        f"Pylint must cap a module at {MODULE_LINE_BUDGET} lines; "
        f"got {pylint_main.get('max-module-lines')!r}"
    )
    assert "too-many-lines" in enable, (
        "the budget must be reported by Pylint's own too-many-lines message"
    )
    assert "syntax-error" in enable, (
        "a module the pass cannot parse must be reported, never skipped in "
        "silence: it would escape the line budget and every other message"
    )
    assert "syntax-error" not in disable, (
        "syntax-error must not be disabled, or an unreadable module lints clean"
    )

    makefile = (REPOSITORY_ROOT / "Makefile").read_text(encoding="utf-8")
    assert "pylint -j $(PYLINT_JOBS)" in makefile, (
        "the Pylint pass must bound its worker pool through PYLINT_JOBS"
    )
    assert "PYLINT_JOBS ?=" in makefile, (
        "the worker pool must be configured rather than left to Pylint"
    )
    assert "check_py_file_lengths" not in makefile, (
        "the line budget must be Pylint's, not re-implemented beside it"
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
    """Active workflows must not select pre-3.14 Python runtimes."""
    workflow = yaml.safe_load(workflow_path.read_text(encoding="utf-8"))
    for setting, value in _workflow_version_values(workflow):
        assert LOW_PYTHON_VERSION.search(str(value)) is None, (
            f"{workflow_path.name}:{setting} configures an obsolete Python "
            f"runtime: {value!r}"
        )


def test_low_python_version_matches_every_pre_314_minor() -> None:
    """The obsolete-runtime matcher must cover Python 3.0 through 3.13."""
    for minor in range(14):
        version = f"3.{minor}"
        assert LOW_PYTHON_VERSION.fullmatch(version), version


@pytest.mark.parametrize("version", ["3.14", "13.12", "3.140"])
def test_low_python_version_preserves_version_boundaries(version: str) -> None:
    """The obsolete-runtime matcher must reject newer and embedded versions."""
    assert LOW_PYTHON_VERSION.search(version) is None, version
