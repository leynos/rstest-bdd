"""Contract that the Linux coverage lanes prove the suite needs no network.

Offline mode makes Cargo refuse to *use* the network. It does not make the
network unreachable, so a suite that passes with offline mode alone has
shown only that nothing asked. These lanes select the ``offline-proof``
Nextest profile, which runs every test inside a private network namespace,
so a green run proves the suite needs none.

Three things have to hold for that proof to mean anything, and each is
pinned here: the profile wraps every Linux test, the lane can actually
start the wrapper (Ubuntu refuses unprivileged user namespaces by default),
and the compiler cache stays reachable from inside the namespace.

Run via ``make test-workflow-contracts``.
"""

import tomllib
import typing as typ

import pytest
from offline_test_contract_test import COVERAGE_ACTION, ROOT, WORKFLOWS
from workflow_support import jobs, steps

#: The wrapper and profile that prove the suite needs no network.
WRAPPER_NAME = "no-network"
PROOF_PROFILE = "offline-proof"
WRAPPER_COMMAND = ["unshare", "--net", "--map-current-user"]

#: The AppArmor knob that decides whether an unprivileged user namespace can
#: start at all on the Ubuntu runner images.
USERNS_RESTRICTION = "kernel.apparmor_restrict_unprivileged_userns"


@pytest.fixture(scope="module")
def nextest_configuration() -> dict[str, typ.Any]:
    """Return the parsed ``.config/nextest.toml``, read once for the module.

    Returns
    -------
    dict[str, typ.Any]
        The configuration, whose wrapper and proof-profile tables are the
        subject of this module.
    """
    return tomllib.loads(
        (ROOT / ".config" / "nextest.toml").read_text(encoding="utf-8")
    )


def _is_windows_job(job: dict[str, typ.Any]) -> bool:
    """Whether *job* is pinned to a Windows runner."""
    return "windows" in str(job.get("runs-on", "")).lower()


def _is_windows_step(step: dict[str, typ.Any]) -> bool:
    """Whether *step* is gated on Windows by its own condition."""
    return "runner.os == 'Windows'" in str(step.get("if", ""))


def _test_running_coverage_steps() -> list[
    tuple[str, str, dict[str, typ.Any], dict[str, typ.Any]]
]:
    """Return every ``(workflow, job, job document, step)`` that runs the suite.

    A step carrying ``build-only`` compiles the instrumented tests without
    running them: nextest never starts, so the proof profile would reach
    nothing there and the lane correctly leaves it alone.
    """
    found: list[tuple[str, str, dict[str, typ.Any], dict[str, typ.Any]]] = []
    for workflow_name in WORKFLOWS:
        for job_name, job_document in jobs(workflow_name).items():
            if "steps" not in job_document:
                continue
            if _is_windows_job(job_document):
                continue
            for step in steps(job_document):
                if not str(step.get("uses", "")).startswith(COVERAGE_ACTION):
                    continue
                if _is_windows_step(step):
                    continue
                if str(step.get("with", {}).get("build-only", "")).lower() == "true":
                    continue
                found.append((workflow_name, job_name, job_document, step))
    return found


#: Every Linux coverage step that runs the suite, computed once so each test
#: is parametrized over the same set.
_LANES = _test_running_coverage_steps()


def _lane_ids(value: typ.Any) -> str | None:
    """Render one lane as ``workflow:job`` for the test list."""
    if isinstance(value, tuple):
        return f"{value[0]}:{value[1]}:{value[3].get('name', '')}"
    return None


def test_the_network_wrapper_is_declared(
    nextest_configuration: dict[str, typ.Any],
) -> None:
    """The wrapper is one unshare invocation in a fresh network namespace."""
    experimental = nextest_configuration.get("experimental", [])
    assert "wrapper-scripts" in experimental, (
        "nextest.toml must opt into wrapper-scripts, or [scripts.wrapper.*] is "
        "inert and no test runs in a namespace"
    )

    wrapper = (
        nextest_configuration.get("scripts", {}).get("wrapper", {}).get(WRAPPER_NAME)
    )
    assert wrapper is not None, (
        f"nextest.toml must declare [scripts.wrapper.{WRAPPER_NAME}], or the "
        "proof profile names a wrapper nothing defines"
    )
    assert wrapper.get("command") == WRAPPER_COMMAND, (
        "the wrapper must be `unshare --net --map-current-user`: `--net` is "
        "the denial, and `--map-current-user` keeps the invoking identity so "
        "no test needs root"
    )


def test_the_proof_profile_wraps_every_linux_test(
    nextest_configuration: dict[str, typ.Any],
) -> None:
    """The proof profile runs all tests, on Linux, through the wrapper."""
    profile = nextest_configuration.get("profile", {}).get(PROOF_PROFILE)
    assert isinstance(profile, dict), (
        f"nextest.toml must declare [profile.{PROOF_PROFILE}], or the coverage "
        "lanes select a profile that does not exist"
    )
    rules = profile.get("scripts", [])
    assert isinstance(rules, list) and rules, (
        f"profile.{PROOF_PROFILE} must declare a scripts rule, or selecting it "
        "changes nothing"
    )
    assert len(rules) == 1, (
        f"profile.{PROOF_PROFILE} must declare exactly one rule, found "
        f"{len(rules)}: a second rule would wrap some tests twice or not at all"
    )

    rule = rules[0]
    assert rule.get("filter") == "all()", (
        "the proof profile must wrap every test; a narrower filter leaves the "
        f"rest unproven, got {rule.get('filter')!r}"
    )
    assert rule.get("platform") == 'cfg(target_os = "linux")', (
        "the wrapper must be Linux-only: Windows has no kernel denial the "
        f"runner can grant, got {rule.get('platform')!r}"
    )
    assert rule.get("run-wrapper") == WRAPPER_NAME, (
        f"the rule must run the {WRAPPER_NAME} wrapper, got {rule.get('run-wrapper')!r}"
    )


@pytest.mark.parametrize(
    ("workflow_name", "job_name", "step"),
    [(workflow, job, step) for workflow, job, _document, step in _LANES],
    ids=_lane_ids,
)
def test_linux_test_lanes_select_the_proof_profile(
    workflow_name: str,
    job_name: str,
    step: dict[str, typ.Any],
) -> None:
    """Every Linux lane that runs the suite opts into the denial."""
    env = step.get("env", {}) or {}
    assert env.get("NEXTEST_PROFILE") == PROOF_PROFILE, (
        f"{workflow_name}:{job_name} runs the suite on Linux without "
        f"NEXTEST_PROFILE={PROOF_PROFILE}; a lane that does not select the "
        "proof profile proves nothing about the network"
    )


@pytest.mark.parametrize(
    ("workflow_name", "job_name", "job", "step"),
    _LANES,
    ids=_lane_ids,
)
def test_linux_test_lanes_can_start_the_wrapper(
    workflow_name: str,
    job_name: str,
    job: dict[str, typ.Any],
    step: dict[str, typ.Any],
) -> None:
    """The lane relaxes the userns restriction and proves the wrapper starts."""
    assert (step.get("env", {}) or {}).get("NEXTEST_PROFILE") == PROOF_PROFILE, (
        "the proof-profile check above must run first; this test explains it"
    )
    runs = "\n".join(str(candidate.get("run", "")) for candidate in steps(job))
    assert USERNS_RESTRICTION in runs, (
        f"{workflow_name}:{job_name} selects the proof profile but never "
        f"relaxes {USERNS_RESTRICTION}, so every test would fail to start"
    )
    assert "unshare --net --map-current-user true" in runs, (
        f"{workflow_name}:{job_name} must prove the wrapper starts before the "
        "suite does; without that, an unrunnable wrapper looks like a failure "
        "of the tests rather than of the lane"
    )


@pytest.mark.parametrize(
    ("workflow_name", "job_name", "job", "step"),
    _LANES,
    ids=_lane_ids,
)
def test_the_compiler_cache_is_reachable_from_the_namespace(
    workflow_name: str,
    job_name: str,
    job: dict[str, typ.Any],
    step: dict[str, typ.Any],
) -> None:
    """A lane that compiles through sccache reaches it over a filesystem socket.

    A private network namespace has its own loopback, so sccache's default TCP
    port is unreachable from inside it. A Unix socket path lives in the shared
    filesystem, so the wrapper keeps the cache.
    """
    if (step.get("env", {}) or {}).get("NEXTEST_PROFILE") != PROOF_PROFILE:
        return
    runs = "\n".join(str(candidate.get("run", "")) for candidate in steps(job))
    if "RUSTC_WRAPPER=" not in runs:
        return
    assert "SCCACHE_SERVER_UDS=" in runs, (
        f"{workflow_name}:{job_name} runs the suite through the namespace "
        "wrapper and compiles through sccache, so it must export "
        "SCCACHE_SERVER_UDS; the default TCP port does not exist inside the "
        "namespace"
    )
