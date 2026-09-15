"""Contract tests for the staged published-GPUI E2E fixture lockfile.

The published-GPUI end-to-end fixture resolves against `cargo package`
artefacts under `target/published-gpui-e2e/`, so the discovery-based
`scripts/check_fixture_lockfiles.py` gate cannot see it: those artefacts only
exist after `make stage-published-gpui-e2e` runs and the manifest's patch
paths point outside the fixture. These tests pin the Makefile contract that
keeps that fixture inside the aggregate lockfile gates instead: a dedicated
`cargo metadata --locked` target for the staged fixture, wired into both
`check-fixture-lockfiles` and `update-fixture-lockfiles`, so a dependency
bump can never stale the fixture's `cargo test --locked` run.
"""

import re
import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the test invokes the trusted local Makefile.
import tomllib
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
MAKEFILE_PATH = REPO_ROOT / "Makefile"
E2E_FIXTURE_DIR = "tests/fixtures/published-gpui-e2e"
PUBLISHED_GPUI_E2E_DIR = "PUBLISHED_GPUI_E2E_DIR"
E2E_LOCKFILE = REPO_ROOT / E2E_FIXTURE_DIR / "Cargo.lock"
E2E_MANIFEST = REPO_ROOT / E2E_FIXTURE_DIR / "Cargo.toml"
ROOT_MANIFEST = REPO_ROOT / "Cargo.toml"


def target_text(makefile: str, target: str) -> str:
    """Return the Makefile slice from *target*'s rule to the next blank line."""
    start = makefile.index(f"{target}: ")
    end = makefile.index("\n\n", start)
    return makefile[start:end]


def target_dependencies(makefile: str, target: str) -> str:
    """Return the prerequisite list of *target*'s Makefile rule."""
    rule_line = target_text(makefile, target).splitlines()[0]
    return rule_line.split(":", 1)[1].strip()


#: One `[[package]]` entry for `ctor`, matching its recorded version.
#: `\r?\n` keeps the pattern working on a CRLF checkout.
CTOR_ENTRY = re.compile(r'(?m)^name = "ctor"\r?\nversion = "(?P<version>[0-9.]+)"')

#: A version below anything the staged crate can require, and below every
#: version the lockfile records, so demoting to it is unambiguous drift.
DEMOTED_VERSION = "0.0.1"


def _version_key(version: str) -> tuple[int, ...]:
    """Return a sortable key for a dotted numeric version.

    Parameters
    ----------
    version : str
        A version such as ``"1.0.13"``.

    Returns
    -------
    tuple[int, ...]
        Its components, so ``"1.0.13"`` sorts above ``"0.4.3"`` rather than
        below it as a string comparison would have them.

    Examples
    --------
    >>> _version_key("1.0.13") > _version_key("0.4.3")
    True
    """
    return tuple(int(part) for part in version.split("."))


def run_staged_fixture_gate() -> subprocess.CompletedProcess[str]:
    """Run the staged fixture gate through the local Make executable."""
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - the local Make executable is trusted.
        [shutil.which("make") or "make", "check-published-gpui-e2e-lock"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
        timeout=900,
    )


def test_staging_version_matches_the_workspace_version() -> None:
    """The package extraction target tracks the workspace release version."""
    workspace = tomllib.loads(ROOT_MANIFEST.read_text(encoding="utf-8"))
    workspace_version = workspace["workspace"]["package"]["version"]

    assert (
        f"PUBLISHED_GPUI_E2E_VERSION := {workspace_version}"
        in MAKEFILE_PATH.read_text(encoding="utf-8")
    ), "the staged package version must match the root workspace version"


def test_fixture_dependencies_match_the_workspace_version() -> None:
    """The direct rstest-bdd requirements track the workspace release."""
    workspace = tomllib.loads(ROOT_MANIFEST.read_text(encoding="utf-8"))
    fixture = tomllib.loads(E2E_MANIFEST.read_text(encoding="utf-8"))
    workspace_version = workspace["workspace"]["package"]["version"]
    package_names = (
        "rstest-bdd",
        "rstest-bdd-harness",
        "rstest-bdd-harness-gpui",
        "rstest-bdd-macros",
    )
    actual_versions = {
        package: fixture["dependencies"][package] for package in package_names
    }

    assert actual_versions == dict.fromkeys(package_names, workspace_version), (
        "the fixture's direct rstest-bdd requirements must match the workspace version"
    )


def test_fixture_patch_paths_match_the_workspace_version() -> None:
    """Every staged crate patch names an artefact from the current release."""
    workspace = tomllib.loads(ROOT_MANIFEST.read_text(encoding="utf-8"))
    fixture = tomllib.loads(E2E_MANIFEST.read_text(encoding="utf-8"))
    workspace_version = workspace["workspace"]["package"]["version"]
    package_names = (
        "rstest-bdd",
        "rstest-bdd-harness",
        "rstest-bdd-harness-gpui",
        "rstest-bdd-macros",
        "rstest-bdd-patterns",
        "rstest-bdd-policy",
    )
    expected_patches = {
        package: {
            "path": f"../../../target/published-gpui-e2e/{package}-{workspace_version}"
        }
        for package in package_names
    }

    assert fixture["patch"]["crates-io"] == expected_patches, (
        "the fixture's patch paths must name staged artefacts for the workspace version"
    )


def test_check_target_depends_on_the_staged_fixture_gate() -> None:
    """The aggregate check validates the staged fixture lockfile too."""
    makefile = MAKEFILE_PATH.read_text(encoding="utf-8")

    assert "check-published-gpui-e2e-lock" in target_dependencies(
        makefile, "check-fixture-lockfiles"
    ), (
        "make check-fixture-lockfiles must run the staged published-GPUI E2E "
        "lockfile gate so the fixture cannot escape the aggregate check"
    )


def test_update_target_depends_on_the_staged_fixture_refresh() -> None:
    """The aggregate refresh regenerates the staged fixture lockfile too."""
    makefile = MAKEFILE_PATH.read_text(encoding="utf-8")

    assert "update-published-gpui-e2e-lock" in target_dependencies(
        makefile, "update-fixture-lockfiles"
    ), (
        "make update-fixture-lockfiles must regenerate the staged published-GPUI "
        "E2E lockfile so the Dependabot workflow refreshes the complete set"
    )


def test_staged_fixture_gate_stages_before_locked_metadata() -> None:
    """The staged fixture gate stages first, then validates with --locked."""
    makefile = MAKEFILE_PATH.read_text(encoding="utf-8")

    assert "stage-published-gpui-e2e" in target_dependencies(
        makefile, "check-published-gpui-e2e-lock"
    ), (
        "the staged fixture gate must run staging before Cargo resolves the "
        "patched manifest"
    )
    assert (
        f"cd $({PUBLISHED_GPUI_E2E_DIR}) && $(CARGO) metadata --locked"
        in target_text(makefile, "check-published-gpui-e2e-lock")
    ), (
        "the staged fixture gate must validate the committed lockfile with "
        "cargo metadata --locked"
    )


def test_e2e_target_keeps_its_locked_test_command() -> None:
    """The end-to-end target still stages and runs `cargo test --locked`."""
    makefile = MAKEFILE_PATH.read_text(encoding="utf-8")
    target = target_text(makefile, "e2e-published-gpui")
    rule_line, recipe = target.split("\n", 1)

    assert "stage-published-gpui-e2e" in rule_line, (
        "the e2e target must keep staging its artefacts before the test run"
    )
    assert f"cd $({PUBLISHED_GPUI_E2E_DIR}) && RUSTFLAGS=" in recipe, (
        "the e2e target must keep running Cargo from the fixture directory"
    )
    assert "$(CARGO) test --locked" in recipe, (
        "the e2e target must keep its locked test command"
    )


def test_staged_fixture_gate_fails_when_the_lockfile_is_stale() -> None:
    """A stale staged lockfile fails the dedicated gate, so the check fails."""
    # Simulate the drift this fix exists to catch: demote the recorded `ctor`
    # version below the staged crate's requirement, which is exactly the state
    # a dependency bump leaves the committed lockfile in. Cargo refuses to
    # update the lockfile under --locked, so the gate must fail before
    # `make test` reaches any behavioural test. The original bytes are
    # restored in a finally block, keeping the mutation invisible to the rest
    # of the suite.
    original_bytes = E2E_LOCKFILE.read_bytes()
    original = original_bytes.decode(encoding="utf-8")
    # `\r?\n` because a Windows checkout may hold the lockfile with CRLF
    # endings, where a pattern spelling the separator `\n` matches nothing.
    entries = list(CTOR_ENTRY.finditer(original))
    assert entries, "the lockfile must record a ctor package version"
    # Exactly one entry, because the lockfile records two `ctor` versions and
    # demoting both to one value leaves the same package specified twice,
    # which Cargo rejects as malformed rather than as stale. The highest is
    # chosen so the demotion is unambiguously downward; measured on this
    # lockfile, demoting either entry does reach Cargo's locked refusal.
    highest = max(entries, key=lambda entry: _version_key(entry["version"]))
    stale_text = (
        original[: highest.start("version")]
        + DEMOTED_VERSION
        + original[highest.end("version") :]
    )
    assert stale_text != original, (
        "the demotion must change the lockfile; a rewrite that lands on the "
        "recorded version leaves the gate validating an unmodified tree"
    )
    try:
        E2E_LOCKFILE.write_text(stale_text, encoding="utf-8", newline="")
        gate = run_staged_fixture_gate()
    finally:
        E2E_LOCKFILE.write_bytes(original_bytes)

    assert gate.returncode != 0, (
        "a stale staged fixture lockfile must fail the dedicated gate"
    )
    # The reason, not just the exit code. Every earlier step can fail
    # non-zero too, and a lockfile this test corrupted rather than demoted
    # fails on a parse error while proving nothing about `--locked`. This
    # phrase appears only in Cargo's refusal to update a locked file.
    assert "--locked was passed" in gate.stdout + gate.stderr, (
        f"the gate must fail on Cargo's refusal to update a locked file, not "
        f"on something earlier; it printed {gate.stdout + gate.stderr!r}"
    )
    gate_recipe_line = target_text(
        MAKEFILE_PATH.read_text(encoding="utf-8"),
        "check-published-gpui-e2e-lock",
    ).splitlines()[-1]
    assert gate_recipe_line.endswith(
        "$(CARGO) metadata --locked --format-version 1 >/dev/null"
    ), "the gate must keep validating through Cargo's --locked metadata check"
