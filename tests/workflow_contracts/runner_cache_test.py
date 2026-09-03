"""Verify the archive cache ownership, key, and save-policy contracts.

Ubicloud destroys each runner's disk, so every cache is an archive with
exactly one owner, one explainable key, and one writer on trunk.

Run with:

    pytest tests/workflow_contracts/runner_cache_test.py
"""

import itertools
from pathlib import PurePosixPath

from workflow_support import (
    CACHE_ACTION_REF,
    ROOT,
    SHARED_SETUP_RUST_CACHE_PROVIDER_HEAD,
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
    path_components as _path_components,
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

GITHUB_SCRIPT_REF = "@ed597411d8f924073f98dfc5c65a23a2325f34cd"
SCCACHE_LOCAL_VARIABLE = "vars.RSTEST_BDD_SCCACHE_LOCAL"
# One job executes the workspace suite, so one job owns the caches.
CACHING_JOBS = (("ci.yml", "build-test"),)
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


def test_one_pinned_cache_action_serves_every_lane() -> None:
    """Ubicloud's transparent cache intercepts this actions/cache revision."""
    for workflow_name, job_name in CACHING_JOBS:
        cache_steps = [
            step
            for step in _steps(_job(workflow_name, job_name))
            if _is_cache_step(step)
        ]
        assert cache_steps, f"{workflow_name}:{job_name} must declare cache steps"
        for step in cache_steps:
            uses = str(step.get("uses"))
            assert uses.endswith(CACHE_ACTION_REF), (
                f"{step.get('name')!r} must pin actions/cache v6.1.0, the "
                "revision whose Linux keys were observed in the Ubicloud cache "
                "listing on 2026-09-03"
            )
        assert not any(
            "ubicloud/cache" in str(step.get("uses", ""))
            for step in _steps(_job(workflow_name, job_name))
        ), f"{workflow_name}:{job_name} must not use the deprecated ubicloud fork"


def test_no_job_archives_a_target_tree() -> None:
    """One compiler cache owns every compiler output and build shape."""
    for workflow_name, job_name in CACHING_JOBS:
        for step in _steps(_job(workflow_name, job_name)):
            if not _is_cache_step(step):
                continue
            for path in _cache_paths(step):
                assert "target" not in _path_components(path), (
                    f"{workflow_name}:{step.get('name')!r} must not archive "
                    f"{path!r}; a target component at any depth is a build "
                    "tree, and sccache owns the debug, cranelift, and "
                    "instrumented objects in one store keyed by flags"
                )


def test_shared_actions_never_own_a_second_cache() -> None:
    """The caller owns caching in every workflow that calls a shared action."""
    workflow_directory = ROOT / ".github" / "workflows"
    for workflow_path in sorted(workflow_directory.glob("*.yml")):
        document = _workflow(workflow_path.name)
        jobs = document.get("jobs")
        assert isinstance(jobs, dict), f"{workflow_path.name} must declare jobs"
        for job_name, job_document in jobs.items():
            if "steps" not in job_document:
                continue
            for step in _steps(job_document):
                uses = str(step.get("uses", ""))
                if not any(
                    uses.startswith(f"leynos/shared-actions/.github/actions/{name}@")
                    for name in ("setup-rust", "generate-coverage")
                ):
                    continue
                inputs = step.get("with")
                assert isinstance(inputs, dict), (
                    f"{workflow_path.name}:{job_name}:{step.get('name')!r} must "
                    "declare inputs so it cedes cache ownership"
                )
                assert inputs.get("cache-provider") == "external", (
                    f"{workflow_path.name}:{job_name}:{step.get('name')!r} must "
                    "set cache-provider: external so the action's own Cargo and "
                    "target archives stay disabled"
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
        and str(step.get("uses")).endswith(f"restore{CACHE_ACTION_REF}")
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
        and str(step.get("uses")).endswith(f"save{CACHE_ACTION_REF}")
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


def test_compiler_cache_backend_has_credentials_and_one_fallback() -> None:
    """The GitHub Actions backend needs credentials a run step cannot see."""
    for workflow_name, job_name in CACHING_JOBS:
        job_steps = _steps(_job(workflow_name, job_name))
        export = job_steps[_step_index(job_steps, "Export Actions cache credentials")]
        assert str(export.get("uses", "")).endswith(GITHUB_SCRIPT_REF), (
            f"{workflow_name} must pin the credential re-export action"
        )
        export_inputs = export.get("with")
        assert isinstance(export_inputs, dict), (
            f"{workflow_name} credential re-export must declare a script"
        )
        script = str(export_inputs.get("script", ""))
        for variable in ("ACTIONS_CACHE_URL", "ACTIONS_RUNTIME_TOKEN"):
            assert variable in script, (
                f"{workflow_name} must re-export {variable} before sccache starts"
            )
        assert "ACTIONS_CACHE_SERVICE_V2', ''" in script, (
            f"{workflow_name} must clear ACTIONS_CACHE_SERVICE_V2 so sccache "
            "uses the v1 API that Ubicloud's cache proxy serves"
        )
        assert "ACTIONS_RESULTS_URL" not in script, (
            f"{workflow_name} must not export ACTIONS_RESULTS_URL: it addresses "
            "GitHub's results service, which the proxy does not serve, and "
            "every sccache write failed against it"
        )
        assert f"{SCCACHE_LOCAL_VARIABLE} != 'true'" in str(export.get("if", "")), (
            f"{workflow_name} must skip the re-export in local-directory mode"
        )
        install_index = _step_index(job_steps, "Install prebuilt sccache")
        export_index = _step_index(job_steps, "Export Actions cache credentials")
        assert export_index < install_index, (
            f"{workflow_name} must re-export the credentials before the sccache "
            "server starts, because --zero-stats starts it"
        )
        sccache_cache = job_steps[_step_index(job_steps, "Restore compiler cache")]
        assert f"{SCCACHE_LOCAL_VARIABLE} == 'true'" in str(sccache_cache.get("if")), (
            f"{workflow_name} must own the compiler directory only on the lanes "
            "that use it, so the backend and the archive never both claim it"
        )
