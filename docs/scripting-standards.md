# Scripting standards

Project scripts must prioritize clarity, reproducibility, and testability. The
baseline tooling is Python and the [`uv`](https://github.com/astral-sh/uv)
launcher so that scripts remain dependency‑self‑contained and easy to execute
in Continuous Integration (CI) or locally.

Cyclopts is the default command‑line interface (CLI) framework for new and
updated scripts. This document supersedes prior guidance that recommended Typer
as a default.

## Rationale for adopting Cyclopts

- Environment‑first configuration without glue. Cyclopts reads environment
  variables with a defined prefix (for example, `INPUT_`) and maps them to
  parameters directly. Bash argument assembly, and bespoke parsing, can be
  removed.
- Typed lists and paths from env. Parameters annotated as `list[str]` or
  `list[pathlib.Path]` are populated from whitespace‑ or delimiter‑separated
  environment values. Custom split/trim helpers are unnecessary.
- Clear precedence model. CLI flags override environment variables, which
  override code defaults. Behaviour is predictable in both CI and local runs.
- Small API surface. The API is explicit and integrates cleanly with type
  hints, aiding readability and testing.
- Backwards‑compatible migration. Option aliases and per‑parameter
  environment variable names permit preservation of existing interfaces while
  removing shell glue.

## Language and runtime

- Target Python 3.13 for all new scripts. Older versions may only be used when
  integration constraints require them, and any exception must be documented
  inline.
- Each script starts with an `uv` script block, so runtime and dependency
  expectations travel with the file. Prefer the shebang
  `#!/usr/bin/env -S uv run python` followed by the metadata block shown in the
  example below.
- External processes are invoked via [`plumbum`](https://plumbum.readthedocs.io)
  to provide structured command execution rather than ad‑hoc shell strings.
- File‑system interactions use `pathlib.Path`. Higher‑level operations (for
  example, copying or removing trees) go through the `shutil` standard library
  module.

### Minimal script (no CLI)

```python
#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["plumbum", "cmd-mox"]
# ///

from __future__ import annotations

from pathlib import Path
from plumbum import local
from plumbum.cmd import tofu


def main() -> None:
    project_root = Path(__file__).resolve().parents[1]
    cluster_dir = project_root / "infra" / "clusters" / "dev"
    with local.cwd(cluster_dir):
        tofu["plan"]()


if __name__ == "__main__":
    main()
```

### Cyclopts CLI pattern (environment‑first)

Employ Cyclopts when a script requires parameters, particularly under CI with
`INPUT_*` variables.

```python
#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts>=2.9", "plumbum", "cmd-mox"]
# ///

from __future__ import annotations

from pathlib import Path
from typing import Annotated

import cyclopts
from cyclopts import App, Parameter
from plumbum import local
from plumbum.cmd import tofu

# Map INPUT_<PARAM> → function parameter without additional glue
app = App(config=cyclopts.config.Env("INPUT_", command=False))


@app.default
def main(
    *,
    # Required parameters
    bin_name: Annotated[str, Parameter(required=True)],
    version: Annotated[str, Parameter(required=True)],

    # Optional scalars
    package_name: str | None = None,
    target: str | None = None,
    outdir: Path | None = None,
    dry_run: bool = False,

    # Lists (whitespace/newline separated by default)
    formats: list[str] | None = None,
    man_paths: Annotated[list[Path] | None, Parameter(env_var="INPUT_MAN_PATHS")] = None,
    deb_depends: list[str] | None = None,
    rpm_depends: list[str] | None = None,
):
    name = package_name or bin_name

    project_root = Path(__file__).resolve().parents[1]
    build_dir = (outdir or (project_root / "dist")) / name

    if dry_run:
        print({
            "name": name,
            "version": version,
            "target": target,
            "formats": formats,
            "man_paths": [str(p) for p in (man_paths or [])],
            "deb_depends": deb_depends,
            "rpm_depends": rpm_depends,
            "build_dir": str(build_dir),
        })
        return

    build_dir.mkdir(parents=True, exist_ok=True)
    with local.cwd(build_dir):
        tofu["plan"]()  # replace with real build tooling


if __name__ == "__main__":
    app()
```

Guidance:

- Parameter names should be descriptive and stable. Where a legacy flag name
  must remain available, add an alias:

  ```python
  package_name: Annotated[str | None, Parameter(aliases=["--name"])] = None
  ```

- Where a specific delimiter is required for an environment list (for example,
  comma‑separated `formats`), specify it per parameter:

  ```python
  formats: Annotated[list[str] | None, Parameter(env_var_split=",")] = None
  ```

- Per‑parameter environment names can be pinned for backwards compatibility:

  ```python
  config_out: Annotated[Path | None, Parameter(env_var="INPUT_CONFIG_PATH")] = None
  ```

## Plumbum: command calling and pipelines

### Basics: command calls, capturing output, handling failures

```python
from __future__ import annotations  # Enables postponed annotation evaluation
from plumbum import local
from plumbum.cmd import git, grep

# Capture stdout (raises ProcessExecutionError on non‑zero exit)
last_commit = git["--no-pager", "log", "-1", "--pretty=%H"]().strip()

# Obtain (rc, out, err) without raising
rc, out, err = git["status"].run(retcode=None)
if rc != 0:
    # handle gracefully; err is available for logging
    …

# Pipelines via the | operator
shortlog = (git["--no-pager", "log", "--oneline"] | grep["fix"])()
```

### Working directory and environment management

```python
from pathlib import Path
from plumbum import local

repo_dir = Path(__file__).resolve().parents[1]

with local.cwd(repo_dir):
    tags = local["git"]["tag", "--list"]()

# Temporary env overrides
with local.env(GIT_AUTHOR_NAME="CI", GIT_AUTHOR_EMAIL="ci@example.org"):
    local["git"]["config", "user.name", "CI"]()
```

### Foreground execution and background jobs

```python
from plumbum import FG, BG
from plumbum.cmd import make

# Stream output to terminal
make["-j4"] & FG

# Background process
proc = local["sleep"]["5"] & BG
proc.wait()
```

### Piping stdin and redirecting

```python
from plumbum import local
from plumbum.cmd import sed, git, grep, wc

# Provide stdin explicitly
_, out, _ = sed["-n", "s/^v//p"].run(stdin="v1.2.3\nv2.0.0\n")
assert out.splitlines() == ["1.2.3", "2.0.0"]

# Multi‑stage pipelines
count = (git["--no-pager", "log", "--oneline"] | grep["chore"] | wc["-l"])().strip()
```

## Pathlib: robust path manipulation

### Project roots, joins, and ensuring directories

```python
from __future__ import annotations  # Enables postponed annotation evaluation
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
DIST = PROJECT_ROOT / "dist"
(DIST / "artifacts").mkdir(parents=True, exist_ok=True)

# Portable joins and normalization
cfg = PROJECT_ROOT.joinpath("config", "release.toml").resolve()
```

### Reading and writing files and atomic updates

```python
from pathlib import Path
import tempfile

f = Path("./dist/version.txt")

# Text I/O
f.write_text("1.2.3\n", encoding="utf-8")
version = f.read_text(encoding="utf-8").strip()

# Atomic write pattern (tmp → replace)
with tempfile.NamedTemporaryFile("w", delete=False, dir=f.parent, encoding="utf-8") as tmp:
    tmp.write("new-contents\n")
    tmp_path = Path(tmp.name)

tmp_path.replace(f)  # atomic on POSIX
```

### Globbing, filtering, and safe deletion

```python
from pathlib import Path

# Recursive glob
md_files = sorted(Path("docs").glob("**/*.md"))

# Filter by suffix / size
small_md = [p for p in md_files if p.stat().st_size < 4096 and p.suffix == ".md"]

# Safe deletion (ignore missing)
try:
    (Path("build") / "temp.bin").unlink()
except FileNotFoundError:
    pass
```

## Cyclopts + Plumbum + Pathlib together (reference script)

```python
#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts>=2.9", "plumbum", "cmd-mox"]
# ///

from __future__ import annotations
from pathlib import Path
from typing import Annotated

import cyclopts
from cyclopts import App, Parameter
from plumbum import local, FG
from plumbum.cmd import git

app = App(config=cyclopts.config.Env("INPUT_", command=False))

@app.default
def main(
    *,
    bin_name: Annotated[str, Parameter(required=True)],
    version: Annotated[str, Parameter(required=True)],
    formats: list[str] | None = None,
    outdir: Path | None = None,
    dry_run: bool = False,
):
    project_root = Path(__file__).resolve().parents[1]
    dist = (outdir or (project_root / "dist")) / bin_name
    dist.mkdir(parents=True, exist_ok=True)

    if not dry_run:
        with local.cwd(project_root):
            (git["tag", f"v{version}"] & FG)

    print({
        "bin_name": bin_name,
        "version": version,
        "formats": formats or [],
        "dist": str(dist),
    })

if __name__ == "__main__":
    app()
```

## Testing expectations

- Automated coverage via `pytest` is required for every script. Fixtures from
  `pytest-mock` support Python‑level mocking; `cmd-mox` simulates external
  executables without touching the host system.
- Behavioural flows that map cleanly to scenarios should adopt Behaviour‑Driven
  Development (BDD) via `pytest-bdd` so that intent is captured in
  human‑readable Given/When/Then narratives.
- Tests reside in `scripts/tests/`, mirroring script names. For example,
  `scripts/bootstrap_doks.py` pairs with `scripts/tests/test_bootstrap_doks.py`.
- Where scripts rely on environment variables, both happy paths and failure
  modes must be asserted; tests should demonstrate graceful error handling
  rather than opaque stack traces.

### Mocking Python dependencies (pytest-mock) and environment (monkeypatch)

```python
import os
from pathlib import Path
from cyclopts.testing import invoke
from scripts.package import app


def test_reads_env_and_defaults(monkeypatch, tmp_path):
    # Arrange env for Cyclopts
    monkeypatch.setenv("INPUT_BIN_NAME", "demo")
    monkeypatch.setenv("INPUT_VERSION", "1.2.3")
    monkeypatch.setenv("INPUT_FORMATS", "deb rpm")  # whitespace or newlines

    # Exercise
    result = invoke(app, [])

    # Assert
    assert result.exit_code == 0
    assert '"version": "1.2.3"' in result.stdout


def test_patch_python_dependency(mocker):
    # Example: patch a helper function used by the script
    from scripts import helpers

    mocker.patch_object(helpers, "compute_checksum", return_value="deadbeef")
    assert helpers.compute_checksum(b"abc") == "deadbeef"
```

### Mocking external executables with cmd-mox (record → replay → verify)

Enable the plugin in `conftest.py`:

```python
pytest_plugins = ("cmd_mox.pytest_plugin",)
```

```python
from plumbum import local


def test_git_tag_happy_path(cmd_mox, monkeypatch, tmp_path):
    monkeypatch.chdir(tmp_path)

    # Mock external command behaviour
    cmd_mox.mock("git").with_args("tag", "v1.2.3").returns(exit_code=0)

    # Run the code under test while shims are active
    cmd_mox.replay()
    local["git"]["tag", "v1.2.3"]()
    cmd_mox.verify()


def test_git_tag_failure_surface_error(cmd_mox, monkeypatch, tmp_path):
    monkeypatch.chdir(tmp_path)

    cmd_mox.mock("git").with_args("tag", "v1.2.3").returns(exit_code=1, stderr="denied")

    cmd_mox.replay()
    try:
        local["git"]["tag", "v1.2.3"]()  # raises due to non‑zero rc
        assert False, "expected ProcessExecutionError"
    except Exception as exc:
        assert "Command exited with code" in str(exc)
    finally:
        cmd_mox.verify()
```

### Spies and passthrough capture (turn real calls into fixtures)

```python
from plumbum import local


def test_spy_and_record(cmd_mox, monkeypatch, tmp_path):
    monkeypatch.chdir(tmp_path)

    # Spy records actual usage; passthrough runs the real command
    spy = cmd_mox.spy("echo").passthrough()

    cmd_mox.replay()
    (local["echo"]["hello world"])()
    cmd_mox.verify()

    # Inspect what happened
    spy.assert_called()
    assert spy.call_count == 1
    args = spy.invocations[0].argv[1:]
    assert args == ["hello world"]
```

## Operational guidelines

- Scripts must be idempotent. Re‑running should converge state without
  destructive side effects. Guard conditions (for example, checking the secrets
  manager to confirm existing secrets) should precede writes or rotations.
- Pure functions that accept configuration objects are preferred over global
  state, so tests can exercise logic deterministically.
- Exit codes should follow Portable Operating System Interface (POSIX)
  conventions: `0` for success, non-zero for actionable failures.
  Human-friendly error messages should highlight remediation steps.
- Dependencies must remain minimal. Any new package should be added to the `uv`
  block and the rationale documented within the script or companion tests.

## Migration guidance (Typer → Cyclopts)

1. Dependencies: replace Typer with Cyclopts in the script’s `uv` block.
2. Entry point: replace `app = typer.Typer(…)` with `app = App(…)` and
   configure `Env("INPUT_", command=False)` where environment variables are
   authoritative in CI.
3. Parameters: replace `typer.Option(…)` with annotations and
   `Parameter(…)`. Mark required options with `required=True`. Map any
   non‑matching environment names via `env_var=…`.
4. Lists: remove custom split/trim code. Use list‑typed parameters; add
   `env_var_split=","` where a non‑whitespace delimiter is required.
5. Compatibility: retain legacy flag names using `aliases=["--old-name"]`.
6. Bash glue: delete argument arrays and conditional appends in GitHub
   Actions. Export `INPUT_*` environment variables and call `uv run` on the
   script.

## CI wiring: GitHub Actions (Cyclopts‑first)

```yaml
- name: Build
  shell: bash
  working-directory: ${{ inputs.project-dir }}
  env:
    INPUT_BIN_NAME: ${{ inputs.bin-name }}
    INPUT_VERSION: ${{ inputs.version }}
    INPUT_FORMATS: ${{ inputs.formats }}               # multiline or space‑sep
    INPUT_OUTDIR: ${{ inputs.outdir }}
  run: |
    set -euo pipefail
    uv run "${GITHUB_ACTION_PATH}/scripts/package.py"
```

## Notes and gotchas

- Newline‑separated lists are preferred for CI inputs to avoid shell quoting
  issues across platforms.
- `Command.run(retcode=None)` permits inspection of non‑zero exits without
  raising.
- Production code should present friendly error messages; tests may assert raw
  behaviours (non‑zero exits, stderr contents) via `cmd-mox`.
- On Windows, newline‑separated lists are recommended for `list[Path]` to
  sidestep `;`/`:` semantics.

This document should be referenced when introducing or updating automation
scripts to maintain a consistent developer experience across the repository.
