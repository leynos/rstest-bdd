"""Contract that every test route fetches its packages before it runs.

A test that spawns Cargo against a cold cache reaches the network, so a
run that is meant to prove the suite needs no network can only do so if
the packages are already present. Two routes have to guarantee that: the
Nextest setup scripts, which run once before any test binary starts, and
the workflow steps, which run ``make prefetch-fixture-deps`` before the
coverage action starts the suite.

The setup-script list is written out literally in ``.config/nextest.toml``
rather than generated, because Nextest reads a static configuration file.
This module is the parity check that keeps the literal list equal to the
manifests the repository actually spawns Cargo against, so a new fixture
cannot be added without a matching prefetch.

Run via ``make test-workflow-contracts``.
"""

import re
import sys
import tomllib
import typing as typ

import pytest
from workflow_support import ROOT, jobs, repository_file, steps

NEXTEST_CONFIG = ROOT / ".config" / "nextest.toml"
SCRIPTS_DIR = ROOT / "scripts"

#: The workspace manifest, which the ordinary build resolves but which the
#: fixture gate deliberately excludes.
ROOT_MANIFEST = "Cargo.toml"

#: A fixture no test spawns Cargo against: ``make check-published-gpui``
#: checks it, and that target is not a test. Prefetching it would cost a
#: network round trip for a package nothing under test needs.
UNSPAWNED_FIXTURE = "tests/fixtures/published-gpui-0-2-2/Cargo.toml"

PREFETCH_STEP = "Prefetch fixture dependencies"
PREFETCH_COMMAND = "make prefetch-fixture-deps"
COVERAGE_ACTION = "leynos/shared-actions/.github/actions/generate-coverage@"
WORKFLOWS = ("ci.yml", "coverage-main.yml")

#: ``setup-scripts`` first appeared in 0.9.98. The floor is asserted rather
#: than the exact pin so a later Nextest release needs no edit here.
MINIMUM_NEXTEST_VERSION = (0, 9, 98)


@pytest.fixture(scope="module")
def nextest_configuration() -> dict[str, typ.Any]:
    """Return the parsed ``.config/nextest.toml``, read once for the module."""
    return tomllib.loads(repository_file(".config", "nextest.toml"))


@pytest.fixture(scope="module")
def expected_manifests() -> set[str]:
    """Return the manifests a test route must have fetched before it runs.

    The set is derived from the repository rather than restated, so a new
    standalone fixture enters it as soon as it is discoverable. The
    discovery module lives under ``scripts/``, which is not on the import
    path of this test tree, so the path is extended here rather than at
    module scope where the import would follow executable code.

    Returns
    -------
    set[str]
        The workspace root manifest plus every discovered fixture manifest,
        excluding the one fixture no test route builds.
    """
    if str(SCRIPTS_DIR) not in sys.path:
        sys.path.insert(0, str(SCRIPTS_DIR))
    from fixture_lockfile_discovery import discover_fixture_manifests

    discovered = {
        path.relative_to(ROOT).as_posix() for path in discover_fixture_manifests(ROOT)
    }
    return {ROOT_MANIFEST} | (discovered - {UNSPAWNED_FIXTURE})


def _setup_scripts(
    configuration: dict[str, typ.Any],
) -> dict[str, dict[str, typ.Any]]:
    """Return the declared ``[scripts.setup.*]`` tables."""
    scripts = configuration.get("scripts", {})
    assert isinstance(scripts, dict), "nextest.toml must declare a scripts table"
    setup = scripts.get("setup", {})
    assert isinstance(setup, dict), "nextest.toml must declare scripts.setup"
    return setup


def _version_tuple(version: str) -> tuple[int, ...]:
    r"""Return *version* as comparable integers.

    A pre-release such as ``0.9.115-rc1`` is rejected by name rather than by a
    bare ``ValueError`` from :func:`int`, so a pinned-but-unparsable version
    says which value broke the comparison. The digit class is ASCII-only
    because ``\d`` would accept digits :func:`int` cannot parse.

    Returns
    -------
    tuple[int, ...]
        One integer per dot-separated part, most significant first.
    """
    assert re.fullmatch(r"[0-9]+(?:\.[0-9]+)*", version), (
        f"nextest-version {version!r} must be dot-separated integers; "
        "a pre-release or range cannot be compared against the floor"
    )
    return tuple(int(part) for part in version.split("."))


def _runs_the_suite(step: dict[str, typ.Any]) -> bool:
    """Whether *step* runs the workspace suite rather than only compiling it.

    ``build-only`` steps warm the compiler cache for another lane; nextest's
    timers never start and no test is executed, so neither a prefetch nor an
    offline proof means anything there.

    Returns
    -------
    bool
        True when the step starts the suite.
    """
    if not str(step.get("uses", "")).startswith(COVERAGE_ACTION):
        return False
    return str(step.get("with", {}).get("build-only", "")).lower() != "true"


def _coverage_jobs() -> list[tuple[str, str]]:
    """Return every ``(workflow, job)`` that starts the workspace suite."""
    found: list[tuple[str, str]] = []
    for workflow_name in WORKFLOWS:
        for job_name, job_document in jobs(workflow_name).items():
            if "steps" not in job_document:
                continue
            if any(_runs_the_suite(step) for step in steps(job_document)):
                found.append((workflow_name, job_name))
    return found


def test_nextest_opts_into_setup_scripts(
    nextest_configuration: dict[str, typ.Any],
) -> None:
    """The configuration enables the feature the setup scripts need."""
    experimental = nextest_configuration.get("experimental", [])
    assert "setup-scripts" in experimental, (
        "nextest.toml must opt into setup-scripts, or the [scripts.setup.*] "
        "tables are inert and nothing is prefetched"
    )

    version = nextest_configuration.get("nextest-version")
    assert isinstance(version, str), (
        "nextest.toml must pin a nextest-version so an older Nextest refuses "
        "the configuration instead of silently ignoring the setup scripts"
    )
    assert _version_tuple(version) >= MINIMUM_NEXTEST_VERSION, (
        f"setup-scripts needs Nextest {'.'.join(map(str, MINIMUM_NEXTEST_VERSION))} "
        f"or newer, but nextest-version is {version}"
    )


def test_every_setup_script_fetches_offline(
    nextest_configuration: dict[str, typ.Any],
) -> None:
    """Each setup script is a locked, offline fetch of one manifest."""
    scripts = _setup_scripts(nextest_configuration)
    assert scripts, "nextest.toml must declare at least one setup script"

    for name, script in scripts.items():
        command = script.get("command")
        assert isinstance(command, list), (
            f"setup script {name!r} must declare a command"
        )
        assert command[:4] == ["cargo", "fetch", "--locked", "--offline"], (
            f"setup script {name!r} must fetch with --locked --offline, got "
            f"{command}: an unlocked or online fetch would resolve a different "
            "graph, or reach the network, before the tests start"
        )
        assert command[4] == "--manifest-path", (
            f"setup script {name!r} must name its manifest with "
            f"--manifest-path, got {command}"
        )
        assert len(command) == 6, (
            f"setup script {name!r} must name exactly one manifest, got {command}"
        )


def test_setup_scripts_cover_every_spawned_manifest(
    nextest_configuration: dict[str, typ.Any],
    expected_manifests: set[str],
) -> None:
    """The literal setup-script list matches the manifests tests spawn."""
    declared = {
        script["command"][5]
        for script in _setup_scripts(nextest_configuration).values()
    }

    assert declared == expected_manifests, (
        "the setup scripts must prefetch exactly the manifests a test route "
        f"spawns Cargo against; missing {sorted(expected_manifests - declared)}, "
        f"unexpected {sorted(declared - expected_manifests)}"
    )


def test_default_profile_runs_every_setup_script(
    nextest_configuration: dict[str, typ.Any],
) -> None:
    """A rule under the default profile runs the scripts for every test."""
    profile = nextest_configuration.get("profile", {}).get("default", {})
    assert isinstance(profile, dict), "nextest.toml must declare profile.default"
    rules = profile.get("scripts", [])
    assert isinstance(rules, list), "profile.default.scripts must be a list"

    matching = [rule for rule in rules if rule.get("filter") == "all()"]
    assert len(matching) == 1, (
        "profile.default.scripts must declare exactly one rule with "
        f'filter = "all()", found {len(matching)}: a narrower filter would '
        "leave some tests running against an unfetched cache"
    )
    assert set(matching[0].get("setup", [])) == set(
        _setup_scripts(nextest_configuration)
    ), (
        "the default profile's setup rule must name every declared setup "
        "script, or a manifest is fetched by nobody"
    )


def test_the_contract_has_lanes_to_judge() -> None:
    """A finder that matched nothing would pass the ordering check silently."""
    found = _coverage_jobs()

    assert len(found) >= 3, (
        "the prefetch ordering contract needs a coverage lane per platform in "
        f"ci.yml and coverage-main.yml; found {found}"
    )


@pytest.mark.parametrize(("workflow_name", "job_name"), _coverage_jobs())
def test_prefetch_precedes_coverage(workflow_name: str, job_name: str) -> None:
    """The prefetch step runs, unconditionally, before the suite starts."""
    job_steps = steps(jobs(workflow_name)[job_name])
    prefetch = [
        index
        for index, step in enumerate(job_steps)
        if step.get("name") == PREFETCH_STEP
    ]
    # Only the steps that run the suite. A job may compile the instrumented
    # tests before its prefetch, and a `build-only` step starts no test, so
    # counting it would demand an order the lane has no reason to keep.
    coverage = [index for index, step in enumerate(job_steps) if _runs_the_suite(step)]

    assert len(prefetch) == 1, (
        f"{workflow_name}:{job_name} must declare exactly one {PREFETCH_STEP!r} "
        f"step, found {len(prefetch)}"
    )
    assert coverage, f"{workflow_name}:{job_name} must start the workspace suite"

    step = job_steps[prefetch[0]]
    assert step.get("run") == PREFETCH_COMMAND, (
        f"{workflow_name}:{job_name} must prefetch through the shared Make "
        f"target, got {step.get('run')!r}"
    )
    assert "if" not in step, (
        f"{workflow_name}:{job_name} must prefetch unconditionally: a "
        "conditional prefetch leaves the suite running against a cold cache"
    )
    assert prefetch[0] < min(coverage), (
        f"{workflow_name}:{job_name} must prefetch before it starts the "
        "workspace suite, or the tests reach the network"
    )
