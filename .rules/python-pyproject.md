# 1. Overview of `uv` and `pyproject.toml`

Astral's `uv` is a Rust-based project and package manager that uses
`pyproject.toml` as its central configuration file. When you run commands like
`uv init`, `uv sync` or `uv run`, `uv` will:

1. Look for a `pyproject.toml` in the project root and keep a lockfile
   (`uv.lock`) in sync with it.
2. Create a virtual environment (`.venv`) if one does not already exist.
3. Read dependency specifications (and any build-system directives) to install
   or update packages accordingly. (Astral Docs[^1], RidgeRun.ai[^2])

In other words, your `pyproject.toml` drives everything—from metadata to
dependencies to build instructions—without needing `requirements.txt` or a
separate `setup.py` file. (Level Up Coding[^3], Python Packaging[^4])

______________________________________________________________________

## 2. The `[project]` Table (PEP 621)

The `[project]` table is defined by PEP 621 and is now the canonical place to
declare metadata (name, version, authors, etc.) and runtime dependencies. At
minimum, PEP 621 requires `name` to be declared statically. `version` must be
declared statically or listed in `dynamic = ["version"]`:

- `name`
- `version` or `dynamic = ["version"]`

However, you almost always want to include at least the following additional
fields for clarity and compatibility:

```toml
[project]
name = "my_project"            # Project name (PEP 621 requirement)
version = "0.1.0"              # Initial semantic version
description = "A brief overview"       # Short summary
readme = "README.md"           # Path to your README file (automatically included)
requires-python = ">=3.12"     # Restrict Python versions, if needed
license = "MIT"                # SPDX licence expression
license-files = ["LICENSE"]    # Include distributed licence files
authors = [
  { name = "Alice Example", email = "alice@example.org" }
]
keywords = ["uv", "astral", "example"]   # (Optional) for metadata registries
classifiers = [
  "Programming Language :: Python :: 3",
  "Operating System :: OS Independent"
]
dependencies = [
  "requests>=2.25",            # Runtime dependency
  "numpy>=1.23"
]
```

- **`name`:** Must be declared statically per PEP 621. **`version`** must be
  declared statically or provided through `dynamic = ["version"]`. (Python
  Packaging[^4], Reddit[^5])
- **`description` and `readme`:** Although not mandatory, they help with
  indexing and packaging tools; `readme = "README.md"` tells `uv` (and PyPI) to
  include your README as the long description. (Astral Docs[^1], Python
  Packaging[^4])
- **`requires-python`:** Constrains which Python interpreters your package
  supports (e.g. `>=3.12`). (Python Packaging[^4], Reddit[^5])
- **`license`:** Specify a licence as an SPDX expression (for example,
  `license = "ISC"`). If licence files are distributed, list them with
  `license-files` (for example, `license-files = ["LICENSE"]`). Do not use
  the deprecated `License ::` classifiers. (Python Packaging[^4], Reddit[^5])
- **`authors`:** A list of tables with `name` and `email`. Many registries
  (e.g., PyPI) pull this for display. (Python Packaging[^4], Reddit[^5])
- **`keywords` and `classifiers`:** These help search engines and package
  indexes. Classifiers must follow the exact trove list defined by PyPA.
  (Python Packaging[^4], Reddit[^5])
- **`dependencies`:** A list of PEP 508-style requirements (e.g.,
  `"requests>=2.25"`) that define version constraints. `uv sync` installs the
  exact versions resolved and recorded in `uv.lock`, updating the lockfile as
  needed. (Astral Docs[^1], RidgeRun.ai[^2])

______________________________________________________________________

## 3. Runtime vs. development dependencies

`project.dependencies` and `project.optional-dependencies` are defined by PEP
621, while `dependency-groups` is defined by PEP 735. Choosing the right one
decides whether a dependency ships to every end user or only ever exists on a
contributor's machine.

Table 1. Dependency field selection.

| Field | Installed for | Use it for |
| --- | --- | --- |
| `project.dependencies` | Everyone who installs the package | Libraries the shipped code imports at runtime |
| `project.optional-dependencies` | End users who opt into an *extra* | Optional runtime *features* (`package[extra]`) |
| `dependency-groups` | Local development only | Test, lint, type-check, docs, and other tooling |

### Required runtime dependencies — `project.dependencies`

Packages the shipped code imports unconditionally. They are published in the
wheel metadata and installed for every consumer. Use PEP 508 specifiers with
bounded ranges, and add them with `uv add <name>`:

```toml
[project]
dependencies = [
  "httpx>=0.27,<1",
]
```

### Optional runtime features — `project.optional-dependencies`

Published "extras" that an *end user* opts into to enable an optional feature
of the package, requested with `package[extra]` syntax (for example,
`pandas[excel]`). Reach for this only when the extra dependency powers
user-facing functionality that not everyone needs — never for development
tooling. Add them with `uv add --optional <extra>`:

```toml
[project.optional-dependencies]
# Opt-in feature: end users request it with my_project[feature].
feature = [
  "some-runtime-lib>=1.2,<2",
]
```

### Development-time dependencies — `dependency-groups`

Tooling only contributors need: test frameworks, linters, type checkers,
documentation builders, and property or mutation testers. These are
**local-only** — PEP 735 dependency groups are *not* included in published
package metadata (they are not part of the wheel), so they must live here rather
than in `project.optional-dependencies`. Add them with `uv add --dev` (the
`dev` group) or `uv add --group <name>`:

```toml
[dependency-groups]
dev = [
  "pytest<9.1",
  "ruff",
  "ty",
]
```

**`uv` installs the `dev` group automatically by default.** `uv run` and
`uv sync` include the `dev` group with no extra flags, so a bare `uv sync`
gives a contributor the full toolchain. Adjust this with:

- `--no-dev` or `--no-default-groups` to exclude development dependencies (for
  example, when building a wheel or a production install).
- `--group <name>` or `--only-group <name>` to include or isolate a
  non-default group.
- `[tool.uv].default-groups` to change which groups sync by default:

```toml
[tool.uv]
default-groups = ["dev", "docs"]  # or "all"
```

Groups may nest via `{ include-group = "..." }`, and by default `uv` resolves
every group together into a single `uv.lock`, so groups must be mutually
compatible unless you declare incompatible sets explicitly under
`[tool.uv].conflicts`.
(Astral Docs[^6])

> **Rule of thumb:** if an end user needs it to *run* the code, it belongs
> in `project.dependencies` (always) or `project.optional-dependencies`
> (an opt-in feature). If only a contributor needs it to *develop, test,
> lint, type-check, or document* the code, it belongs in
> `dependency-groups`.

______________________________________________________________________

## 4. Entry Points and Scripts

If you want to expose command-line interfaces (CLIs) or GUIs through your
package, PEP 621 provides the `[project.scripts]` and `[project.gui-scripts]`
tables:

```toml
[project.scripts]
mycli = "my_project.cli:main"

[project.gui-scripts]
mygui = "my_project.gui:start"
```

- **`[project.scripts]`:** Defines console scripts. When you run `uv run mycli`,
  `uv` will invoke the `main` function in `my_project/cli.py`. (Astral Docs[^7])
- **`[project.gui-scripts]`:** On Windows, `uv` will wrap these in a GUI
  executable; on Unix-like systems, they behave like normal console scripts.
  (Astral Docs[^7])
- **Plugin Entry Points:** If your project supports plugins, use
  `[project.entry-points.'group.name']` to register them. (Astral Docs[^7])

______________________________________________________________________

## 5. Declaring a Build System

The `[build-system]` table is optional under PEP 517/518. When present, it tells
tools how to build and install your project, and `uv` uses its presence to
determine whether to install the current project. When absent, `uv` falls back
to the legacy setuptools backend. A "modern" convention is to specify
`setuptools>=61.0` (for editable installs without `setup.py`) or a lighter
alternative like `flit_core`. Below is the typical setup using setuptools:

```toml
[build-system]
requires = ["setuptools>=61.0", "wheel"]
build-backend = "setuptools.build_meta"
```

- **`requires`:** A list of packages needed at build time. For editable installs
  in `uv`, you need at least `setuptools>=61.0` and `wheel`. (Python
  Packaging[^4], Astral Docs[^7])
- **`build-backend`:** The entry point for your build backend.
  `setuptools.build_meta` is the PEP 517-compliant backend for setuptools.
  (Python Packaging[^4], Astral Docs[^7])
- **Note:** If you omit `[build-system]`, `uv` falls back to
  `setuptools.build_meta:__legacy__` and installs dependencies, but it will not
  install your own project unless you set `tool.uv.package = true` (see next
  section). (Astral Docs[^7])

______________________________________________________________________

## 6. `uv`-Specific Configuration (`[tool.uv]`)

Astral `uv` allows you to inject its own settings in `[tool.uv]`. The most
common option is:

```toml
[tool.uv]
package = true
```

- **`tool.uv.package = true`:** Forces `uv` to build and install your project
  into its virtual environment every time you run `uv sync` or `uv run`.
  Without this, `uv` only installs dependencies (not your own package) if
  `[build-system]` is missing. (Astral Docs[^7])
- You may also set other `uv`-specific keys (e.g., custom indexes, resolver
  policies) under `[tool.uv]`, but `package` is the most common. (Python
  Packaging[^4], Astral Docs[^7])

______________________________________________________________________

## 7. Putting It All Together: Example `pyproject.toml`

Below is a complete example that demonstrates all sections. Adjust values as
needed for your own project.

```toml
[project]
name = "my_project"
version = "0.1.0"
description = "An illustrative example for Astral uv"
readme = "README.md"
requires-python = ">=3.12"
license = "MIT"
license-files = ["LICENSE"]
authors = [
  { name = "Alice Example", email = "alice@example.org" }
]
keywords = ["astral", "uv", "pyproject", "example"]
classifiers = [
  "Programming Language :: Python :: 3",
  "Operating System :: OS Independent"
]
dependencies = [
  "requests>=2.25",
  "numpy>=1.23"
]

# Opt-in runtime feature; end users install it with my_project[fast].
[project.optional-dependencies]
fast = [
  "orjson>=3.9"
]

# Development-only tooling (PEP 735); never shipped to end users.
[dependency-groups]
dev = [
  "pytest>=7.0",
  "ruff",
  "ty"
]
docs = [
  "sphinx>=5.0",
  "sphinx-rtd-theme"
]

[project.scripts]
mycli = "my_project.cli:main"

[build-system]
requires = ["setuptools>=61.0", "wheel"]
build-backend = "setuptools.build_meta"

[tool.uv]
package = true
```

**Explanation of key points:**

1. **Metadata under `[project]`:**

   - `name` must be declared statically; `version` must be declared statically
     or provided through `dynamic = ["version"]` (PEP 621). (Python Packaging
     [^4], Reddit[^5])
   - `description`, `readme`, `requires-python`: provide clarity about the
     project and help tools like PyPI. (Python Packaging[^4], Reddit[^5])
   - `license`, `authors`, `keywords`, `classifiers`: standardized metadata,
     which improves discoverability. (Python Packaging[^4], Reddit[^5])
   - `dependencies`: runtime requirements, expressed in PEP 508 syntax.
     (Astral Docs[^1], RidgeRun.ai[^2])

2. **Optional features vs. development tooling:**

   - `[project.optional-dependencies]` declares an opt-in runtime *extra*
     (`fast`), installed by end users via `my_project[fast]`. (Python
     Packaging[^4])
   - `[dependency-groups]` (PEP 735) holds development-only tooling (`dev`,
     `docs`) that is never published; `uv sync` installs the `dev` group by
     default. (Astral Docs[^6])

3. **Entry Points (`[project.scripts]`):**

   - Defines a console command `mycli` that maps to `my_project/cli.py:main`.
     Invoking `uv run mycli` will run the `main()` function. (Astral Docs[^7])

4. **Build System:**

   - `setuptools>=61.0` plus `wheel` ensures both legacy and editable installs
     work. ✱ Newer versions of setuptools support PEP 660 editable installs
     without a `setup.py` stub. (Python Packaging[^4], Astral Docs[^7])
   - `build-backend = "setuptools.build_meta"` tells `uv` how to compile your
     package. (Python Packaging[^4], Astral Docs[^7])

5. **`[tool.uv]`:**

   - `package = true` ensures that `uv sync` will build and install your own
     project (in editable mode) every time dependencies change. Otherwise, `uv`
     treats your project as a collection of scripts only (no package). (Astral Docs[^7])

______________________________________________________________________

## 8. Additional Tips & Best Practices

1. **Keep `pyproject.toml` Human-Readable:** Edit it by hand when possible.
   Modern editors (VS Code, PyCharm) offer TOML syntax highlighting and PEP 621
   autocompletion. (Python Packaging[^4])

2. **Lockfile Discipline:** After modifying `dependencies` or any `[project]`
   fields, always run `uv sync` (or `uv lock`) to update `uv.lock`. This
   guarantees reproducible environments. (Astral Docs[^1])

3. **Semantic Versioning (optional project policy):** Python package `version`
   values must comply with [PEP 440](https://peps.python.org/pep-0440/). A
   project may additionally adopt [SemVer](https://semver.org/) for its version
   policy (for example, `1.2.3`), using patch, minor, and major increments for
   bug fixes, backward-compatible changes, and breaking changes respectively.
   (Python Packaging[^4])

4. **Keep Build Constraints Minimal:** If you don't need editable installs, you
   can omit `[build-system]` (but then `uv` won't build your package; it will
   only install dependencies). To override, set `tool.uv.package = true`.
   (Astral Docs[^7])

5. **Use Exact or Bounded Ranges for Dependencies:** Rather than `requests`, use
   `requests>=2.25, <3.0` to avoid unexpected major bumps. (DevsJC[^8])

6. **Consider Dynamic Fields Sparingly:** You can declare fields like
   `dynamic = ["version"]` if your version is computed at build time (e.g. via
   `setuptools_scm`). If you do so, ensure your build backend supports dynamic
   metadata. (Python Packaging[^4])

______________________________________________________________________

## 9. Summary

A "modern" `pyproject.toml` for an Astral `uv` project should:

- Use the PEP 621 `[project]` table for metadata and runtime `dependencies`.
- Declare opt-in runtime *features* as extras under
  `[project.optional-dependencies]`, and development-only tooling under
  `[dependency-groups]` (the `dev` group installs by default).
- Define any CLI or GUI entry points under `[project.scripts]` or
  `[project.gui-scripts]`.
- Declare a PEP 517 `[build-system]` (e.g. `setuptools>=61.0`, `wheel`,
  `setuptools.build_meta`) to support editable installs, or omit it and rely on
  `tool.uv.package = true`.
- Include a `[tool.uv]` section, at minimum `package = true` if you want `uv` to
  build and install your own package.

Following these conventions ensures that your project is fully PEP-compliant,
easy to maintain, and integrates seamlessly with Astral `uv`.

[^1]: [Working on projects | uv - Astral Docs](https://docs.astral.sh/uv/guides/projects/)
[^2]: [UV Tutorial: A Fast Python Package and Project Manager](https://www.ridgerun.ai/post/uv-tutorial-a-fast-python-package-and-project-manager)
[^3]: [Modern Python Development with pyproject.toml and UV](https://levelup.gitconnected.com/modern-python-development-with-pyproject-toml-and-uv-405dfb8b6ec8)
[^4]: [Writing your pyproject.toml – Python Packaging User Guide](https://packaging.python.org/en/latest/guides/writing-pyproject-toml/)
[^5]: [Anyone used UV package manager in production? (Reddit)](https://www.reddit.com/r/Python/comments/1ixryec/anyone_used_uv_package_manager_in_production/)
[^6]: [Managing dependencies | uv - Astral Docs](https://docs.astral.sh/uv/concepts/projects/dependencies/)
[^7]: [Configuring projects | uv - Astral Docs](https://docs.astral.sh/uv/concepts/projects/config/)
[^8]: [The Complete Guide to pyproject.toml – devsjc blogs](https://devsjc.github.io/blog/20240627-the-complete-guide-to-pyproject-toml/)
