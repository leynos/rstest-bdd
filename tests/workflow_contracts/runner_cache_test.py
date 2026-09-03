"""Verify the archive cache ownership, key, and save-policy contracts.

Ubicloud destroys each runner's disk, so every cache is an archive with
exactly one owner, one explainable key, and one writer on trunk.

Run with:

    pytest tests/workflow_contracts/runner_cache_test.py
"""

import itertools
from pathlib import PurePosixPath

from workflow_support import (
    GITHUB_CACHE_REF,
    SHARED_SETUP_RUST_CACHE_PROVIDER_HEAD,
    UBICLOUD_CACHE_PREFIX,
    UBICLOUD_CACHE_REF,
)
from workflow_support import (
    cache_owner as _cache_owner,
)
from workflow_support import (
    cache_paths as _cache_paths,
)
from workflow_support import (
    is_cache_step as _is_cache_step,
)
from workflow_support import (
    job as _job,
)
from workflow_support import (
    step_index as _step_index,
)
from workflow_support import (
    steps as _steps,
)
from workflow_support import (
    workflow as _workflow,
)

TRUNK_SAVE_GUARD_FRAGMENTS = (
    "github.event_name == 'push'",
    "github.ref == 'refs/heads/main'",
    "matrix.features == ''",
)


def test_ci_publishes_a_trusted_cache_generation_from_trunk() -> None:
    """Without a trunk trigger no run may ever populate the trusted cache."""
    workflow = _workflow("ci.yml")
    triggers = workflow.get(True) or workflow.get("on")
    assert isinstance(triggers, dict), "ci.yml must declare a trigger mapping"
    push = triggers.get("push")
    assert isinstance(push, dict), "ci.yml must run on push so a writer exists"
    assert push.get("branches") == ["main"], (
        "ci.yml must publish its trusted cache generation from main only"
    )
    assert "pull_request" in triggers, "ci.yml must still gate pull requests"


def test_cache_actions_are_pinned_per_runner_provider() -> None:
    """Ubicloud lanes use ubicloud/cache; GitHub-hosted lanes use actions/cache."""
    steps = _steps(_job("ci.yml", "build-test"))
    cache_steps = [step for step in steps if _is_cache_step(step)]
    assert cache_steps, "ci.yml:build-test must declare cache steps"
    for step in cache_steps:
        uses = str(step.get("uses"))
        guard = str(step.get("if", ""))
        if uses.startswith(UBICLOUD_CACHE_PREFIX):
            assert uses.endswith(UBICLOUD_CACHE_REF), (
                f"{step.get('name')!r} must pin the reviewed ubicloud/cache commit"
            )
            assert "runner.os == 'Linux'" in guard, (
                f"{step.get('name')!r} must be restricted to Ubicloud Linux lanes: "
                "the action needs runtime variables only an Ubicloud VM supplies"
            )
        else:
            assert uses.endswith(GITHUB_CACHE_REF), (
                f"{step.get('name')!r} must pin the repository-approved "
                "actions/cache commit"
            )
            assert "runner.os == 'Windows'" in guard, (
                f"{step.get('name')!r} must be restricted to the GitHub-hosted lane"
            )


def test_every_cached_path_has_exactly_one_owner() -> None:
    """Two cache steps must never contend for the same directory."""
    steps = _steps(_job("ci.yml", "build-test"))
    owners: dict[str, set[str]] = {}
    for step in steps:
        if not _is_cache_step(step):
            continue
        owners.setdefault(_cache_owner(step), set()).update(_cache_paths(step))
    assert owners, "ci.yml:build-test must declare cache owners"
    for (left_name, left), (right_name, right) in itertools.combinations(
        owners.items(), 2
    ):
        for left_path, right_path in itertools.product(sorted(left), sorted(right)):
            first = PurePosixPath(left_path)
            second = PurePosixPath(right_path)
            contention = (
                f"cache owners {left_name!r} and {right_name!r} both claim "
                f"{left_path!r} and {right_path!r}; each path needs one owner"
            )
            assert first != second, contention
            assert first not in second.parents, contention
            assert second not in first.parents, contention


def test_cache_restores_precede_every_install_and_build() -> None:
    """A cache that lands after its installer saves nothing."""
    steps = _steps(_job("ci.yml", "build-test"))
    checkout_index = _step_index(steps, "Checkout")
    setup_rust_index = _step_index(steps, "Setup Rust")
    restore_indices = [
        index
        for index, step in enumerate(steps)
        if _is_cache_step(step)
        and str(step.get("uses")).endswith((
            f"restore{UBICLOUD_CACHE_REF}",
            f"restore{GITHUB_CACHE_REF}",
        ))
    ]
    assert restore_indices, "ci.yml:build-test must restore its caches"
    assert min(restore_indices) > checkout_index, (
        "cache restores must follow checkout so lockfile hashes are available"
    )
    assert max(restore_indices) < setup_rust_index, (
        "every cache restore must precede toolchain and tool installation"
    )


def test_cache_saves_are_restricted_to_one_writer_on_trunk() -> None:
    """Pull requests read the trusted generation and never publish one."""
    steps = _steps(_job("ci.yml", "build-test"))
    save_steps = [
        step
        for step in steps
        if _is_cache_step(step)
        and str(step.get("uses")).endswith((
            f"save{UBICLOUD_CACHE_REF}",
            f"save{GITHUB_CACHE_REF}",
        ))
    ]
    assert save_steps, "ci.yml:build-test must save its caches from trunk"
    for step in save_steps:
        guard = str(step.get("if", ""))
        for fragment in TRUNK_SAVE_GUARD_FRAGMENTS:
            assert fragment in guard, (
                f"{step.get('name')!r} must restrict saving with {fragment!r} so "
                "one lane on trunk owns each key"
            )
        assert "outputs.cache-hit != 'true'" in guard, (
            f"{step.get('name')!r} must skip saving when the restore already hit"
        )


def test_build_matrix_reports_cache_and_compiler_cache_state() -> None:
    """Expose every rendered key, hit result, and compiler-cache counter."""
    steps = _steps(_job("ci.yml", "build-test"))
    observations = steps[_step_index(steps, "Record cache observations")]
    assert observations.get("if") == "${{ always() }}", (
        "cache observations must be recorded even when the job fails"
    )
    observation_script = str(observations.get("run"))
    for key_name in (
        "CARGO_CACHE_KEY",
        "CI_TOOLS_CACHE_KEY",
        "WHITAKER_CACHE_KEY",
        "SCCACHE_CACHE_KEY",
    ):
        assert key_name in observation_script, (
            f"the cache observation summary must render {key_name}"
        )
    assert "GITHUB_STEP_SUMMARY" in observation_script, (
        "the cache observation summary must reach the job summary"
    )
    sccache_install = steps[_step_index(steps, "Install prebuilt sccache")]
    sccache_install_command = str(sccache_install.get("run"))
    required_fragments = {
        "cargo binstall",
        "--disable-strategies compile",
        "--only-signed",
        "RUSTC_WRAPPER",
        "--zero-stats",
    }
    missing_fragments = sorted(
        fragment
        for fragment in required_fragments
        if fragment not in sccache_install_command
    )
    assert not missing_fragments, (
        f"ci.yml:sccache installer omits required fragments: {missing_fragments}"
    )
    effectiveness = steps[_step_index(steps, "Report compiler-cache effectiveness")]
    effectiveness_script = str(effectiveness.get("run"))
    assert effectiveness.get("if") == "${{ always() }}", (
        "compiler-cache statistics must be reported even when the job fails"
    )
    for required_fragment in ("sccache --show-stats", "--stats-format json"):
        assert required_fragment in effectiveness_script, (
            f"ci.yml must publish {required_fragment!r} for the run ledger"
        )


def test_shared_actions_leave_cache_ownership_to_the_caller() -> None:
    """Only the workflow may own the cached Cargo, uv, and tool paths."""
    build_test = _job("ci.yml", "build-test")
    steps = _steps(build_test)
    setup_step = steps[_step_index(steps, "Setup Rust")]
    assert setup_step.get("uses") == (
        "leynos/shared-actions/.github/actions/setup-rust@"
        f"{SHARED_SETUP_RUST_CACHE_PROVIDER_HEAD}"
    ), "ci.yml:Setup Rust must use the external-cache provider revision"
    assert setup_step.get("with") == {
        "toolchain": "${{ matrix.rust-toolchain }}",
        "cache-provider": "external",
        "use-sccache": "false",
    }, "ci.yml:Setup Rust must leave cache and sccache ownership to the caller"
    coverage_steps = [
        step
        for step in steps
        if str(step.get("uses", "")).startswith(
            "leynos/shared-actions/.github/actions/generate-coverage@"
        )
    ]
    assert coverage_steps, "ci.yml:build-test must generate coverage"
    for step in coverage_steps:
        inputs = step.get("with")
        assert isinstance(inputs, dict), "a coverage step must declare inputs"
        assert inputs.get("cache-provider") == "external", (
            "the coverage action must not become a second cache owner"
        )
        assert not inputs.get("pytest-workers"), (
            "the Python coverage suite must use an explicit worker count, "
            "never an unconstrained -n auto"
        )
