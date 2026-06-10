# Developer guide

For engineers and contributors working on the rstest-bdd codebase.  This guide
covers workspace tooling, test infrastructure, macro internals, and the
patterns used across crates — it is not a user-facing tutorial.

## Workspace dependency policy

Keep workspace-local development and crates.io publication on the same manifest
surface by declaring shared dependencies in the root `[workspace.dependencies]`
table. First-party crates must use both `version` and `path` there, then
consume the dependency with `.workspace = true` from member manifests. The
`path` keeps local builds on the current checkout after a version has been
published, while the `version` gives Cargo the crates.io requirement it needs
when packaging a crate.

Publishable first-party crates must keep their package-time dependency graph
acyclic across normal, build, and development dependencies. `cargo package`
resolves development dependencies while preparing a crate, so a dev-dependency
cycle can block a live release even when the runtime dependency graph is
acyclic. When cross-crate tests need both the runtime API and procedural
macros, place those tests in the crate that already depends downstream, or in
an existing non-publishable example/test harness. Do not add a reverse
dev-dependency to a lower-level crate to host integration coverage.

Use the existing dependency-load-bearing crates before introducing or widening
edges. `rstest-bdd-patterns` owns shared pattern parsing, `rstest-bdd-policy`
owns shared runtime and attribute-policy classification, and
`rstest-bdd-harness` owns adapter contracts and test-staging helpers. The
procedural macro crate may depend on those shared crates, but it must not
depend on `rstest-bdd`; macro/runtime integration tests live under `rstest-bdd`
instead.

Do not restore root-level `[patch.crates-io]` entries for normal development.
Patches make local resolution differ from publish-time resolution and can hide
registry-only failures. If a temporary patch is required for a one-off
diagnostic, remove it before committing or configure `lading.toml` so
`lading publish` strips it from staged release workspaces.

The GPUI test shim follows the same pattern. The workspace dependency for
`gpui` points at `vendor/gpui` with a matching crates.io version, so local
tests use the stable-compatible shim. `lading publish` stages the workspace and
strips local patch entries before packaging, so the staged release surface uses
the upstream `gpui` dependency declaration before publication.

## Staging fixtures for trybuild tests

The `rstest-bdd-harness` crate exposes a `#[doc(hidden)]` module
`trybuild_staging` with two public helpers:

- `copy_file(source, destination)` — copies a single file, creating parent
  directories as needed.
- `copy_dir_tree(source, destination)` — recursively copies a directory tree,
  replacing `destination` if it already exists. Symlinks under `source` are
  rejected with an `InvalidInput` error to prevent escape or copy loops.

Both helpers are intended for use by `macro_compile` integration tests in the
Tokio and GPUI harness crates to stage `.feature` files into the trybuild
scratch directory before `TestCases::pass` / `compile_fail` are called. Do not
use these helpers outside test code.

`copy_dir_tree` rejects overlapping source and destination trees before it
removes or creates the destination. The overlap check canonicalizes
destinations whose final path does not exist yet by walking to the nearest
existing ancestor and replaying the missing tail. Missing parent chains and
parent-directory components such as `missing/../dst` must therefore preserve
their logical meaning, so a destination that resolves back to the source tree
is rejected even when part of the destination path is not yet present.

## Macro expansion snapshot helpers (`macrotest_support`)

The `rstest-bdd-harness` crate exposes a `#[doc(hidden)]` module
`macrotest_support` that provides shared helpers for the `macro_compile`
integration suites in the Tokio and GPUI harness crates. Both suites run
`macrotest` against committed `.expanded.rs` snapshots and need a common way to
gate snapshot refresh, perform substring assertions over snapshot contents, and
resolve per-crate trybuild scratch directories. The module is not part of the
supported public surface of `rstest-bdd-harness`.

### Snapshot refresh gating

`snapshot_refresh_is_enabled()` returns `true` only when the
`RSTEST_BDD_RUN_MACROTEST` environment variable is set and the `cargo expand`
subcommand is available on `PATH`. It gates `macrotest::expand_without_refresh`
calls so snapshot comparisons are skipped in ordinary CI and local development,
and only exercised during deliberate snapshot-refresh workflows.

### Snapshot substring assertions

- `assert_snapshot_contains(path, needles)` — asserts that each needle
  substring appears at least once in the snapshot file at `path`. Panics on I/O
  failure or when any needle is absent from the snapshot contents.
- `assert_snapshot_omits(path, needle)` — asserts that `needle` does not
  appear anywhere in the snapshot file at `path`. Panics on I/O failure or when
  the needle is found in the snapshot contents.

Both functions read the full snapshot into memory and use substring matching,
so they are intended for small, human-readable `.expanded.rs` snapshots.

### Trybuild crate root resolution

`trybuild_crate_root(manifest_path, target_subdir)` resolves the per-crate
trybuild scratch directory by querying `cargo metadata` for the workspace
`target` directory and appending `tests/trybuild/<target_subdir>`. It returns
`Result<PathBuf, Box<dyn Error>>` and is consumed by
`stage_trybuild_support_files` in each harness crate's `macro_compile.rs` test.

## nextest configuration (`.config/nextest.toml`)

cargo-nextest reads its configuration from `.config/nextest.toml` at the
workspace root; this is the only nextest configuration file the runner loads.
The file sets the timeout policy for the test suite:

- The default profile kills any test that runs past a 60 s `slow-timeout`
  (`terminate-after = 1`, 5 s grace period) and applies a 5 m `global-timeout`
  to the whole run.
- A `[[profile.default.overrides]]` entry raises the `slow-timeout` to 180 s
  for `cargo-bdd::cli`, whose smoke tests spawn `cargo` to build fixture
  crates and can legitimately exceed 60 s on cold caches.
- A second override applies the same 180 s `slow-timeout` to the
  trybuild-based compile-test binaries:
  `rstest-bdd-harness-tokio::macro_compile`,
  `rstest-bdd-harness-gpui::macro_compile`, and
  `rstest-bdd::trybuild_macros`. These tests invoke `cargo build` against a
  large dependency tree, so a cold cache (or CPU contention when several
  compile tests run concurrently) can push a single test well past the
  default limit even though nothing is wrong.
- A `long` profile (`--profile long`) relaxes the limits further (180 s
  `slow-timeout`, 15 m `global-timeout`) for deliberately slow local runs.

When adding a test binary that shells out to `cargo`, extend the relevant
override's `filter` expression rather than raising the default
`slow-timeout`: the tight default is what surfaces genuinely hung tests
quickly.

## nextest on Windows: trybuild deadlock

nextest wraps test binaries in Windows Job Objects. Child `cargo` processes
spawned by `trybuild` and `cargo_metadata` inherit the write end of nextest's
capture pipe. Because Windows pipe semantics keep the read end open until all
holders of the write end have closed it, and because rustc spawns many
short-lived child processes that also inherit the handle, the pipe never closes
and nextest waits until its slow-timeout fires.

Mitigation:

- Continuous Integration (CI) sets `use-nextest: false` for all Windows
  matrix legs (see `.github/workflows/ci.yml`). Windows coverage runs use
  `cargo llvm-cov test` (libtest) instead.
- `.config/nextest.toml` raises the `slow-timeout` for the trybuild
  compile-test binaries (including both `macro_compile` binaries) to 180 s as
  a local-development safety net. This does not fix the deadlock; it only
  delays termination to allow the build to complete on fast machines.
- Do not add `macro_compile`-style tests (tests that spawn `cargo` via
  `trybuild` or `cargo_metadata`) to nextest-managed binaries intended to run
  on Windows.

## Users-guide link validation (`scripts/check_users_guide_links.py`)

`docs/users-guide.md` is vendored into consumer projects, so its
cross-references to other documents in this repository use absolute GitHub
URLs (collected as reference-style definitions at the bottom of the file)
rather than relative paths. `scripts/check_users_guide_links.py`, run
automatically by `make lint`, keeps those URLs honest:

- Every repository reference must start with the canonical base URL recorded
  in the script's `BASE_URL` constant (currently
  `https://github.com/leynos/rstest-bdd/blob/main/docs/`). If the repository
  moves, the default branch is renamed, or the documents relocate, update
  that one constant and the reference block; the check pinpoints every
  definition that disagrees.
- Each link must resolve to an existing file under `docs/`, and any `#`
  fragment must match a heading anchor in the target document (the script
  derives anchors with GitHub's slug rules). Prefer heading fragments over
  `#L<n>` line anchors, which silently break on reflows.
- The check also fails if the guide contains no repository references at
  all, so a reformat cannot silently defang it.

Non-repository URLs (for example docs.rs links) are ignored. Unit tests live
under `scripts/tests/test_check_users_guide_links.py`.

## GPUI mapping-table validation (`scripts/check_gpui_mapping_table.py`)

The vendored-to-published GPUI mapping table is duplicated in
`docs/users-guide.md` and `docs/rstest-bdd-design.md`. Update both copies
together whenever a GPUI test API shape changes. `make lint` runs
`scripts/check_gpui_mapping_table.py`, which anchors each table by its
surrounding heading and compares the four data rows after whitespace
normalization, so doc-vs-doc drift fails locally and in Continuous
Integration (CI).

This check does not prove the published column against crates.io. Local
workspace builds resolve `gpui` to `vendor/gpui`, while release validation now
runs through `lading publish`, which strips local patch entries in the staged
workspace. When the workspace bumps GPUI, re-verify the published column from
the published crate source before editing the table. One reproducible path is:

```bash
mkdir -p /tmp/rstest-bdd-gpui-check
curl -L https://static.crates.io/crates/gpui/gpui-${VERSION}.crate \
  -o /tmp/rstest-bdd-gpui-check/gpui-${VERSION}.crate
tar -xf /tmp/rstest-bdd-gpui-check/gpui-${VERSION}.crate \
  -C /tmp/rstest-bdd-gpui-check
sed -n '1,220p' \
  /tmp/rstest-bdd-gpui-check/gpui-${VERSION}/src/app/test_context.rs
```

If the crate's embedded repository commit is needed for cross-checking, inspect
the extracted manifest metadata and compare the relevant files against the Zed
repository with `git show <commit>:crates/gpui/src/app/test_context.rs`.
Unit tests for the table checker live in
`scripts/tests/test_check_gpui_mapping_table.py`.

Both link-checker and table-checker tests run with the Python suite in
`make test`. Issue #537 tracks generating the users-guide reference block from
`BASE_URL` so the base lives in exactly one place.

## Test organization: harness-owned integration tests

Tokio and GPUI harness integration tests are co-located with their respective
harness crates:

Table: Test binaries for `rstest-bdd-harness-tokio` and
`rstest-bdd-harness-gpui`

| Crate                      | Test binary                  | What it tests                                                        |
| -------------------------- | ---------------------------- | -------------------------------------------------------------------- |
| `rstest-bdd-harness-tokio` | `harness_behaviour`          | Tokio harness adapter execution semantics                            |
| `rstest-bdd-harness-tokio` | `attribute_policy_behaviour` | Tokio attribute policy output                                        |
| `rstest-bdd-harness-tokio` | `scenario_macros`            | `#[scenario]` + Tokio adapter                                        |
| `rstest-bdd-harness-tokio` | `macro_compile`              | trybuild compile-pass/fail for Tokio fixtures                        |
| `rstest-bdd-harness-gpui`  | `harness_behaviour`          | GPUI harness adapter execution semantics (feature-gated)             |
| `rstest-bdd-harness-gpui`  | `attribute_policy_behaviour` | GPUI attribute policy output (feature-gated)                         |
| `rstest-bdd-harness-gpui`  | `scenario_macros`            | `#[scenario]` + GPUI adapter (feature-gated)                         |
| `rstest-bdd-harness-gpui`  | `stateful_window`            | durable GPUI handles + visual context reconstruction (feature-gated) |
| `rstest-bdd-harness-gpui`  | `scenario_name_in_logs`      | GPUI step-panic diagnostics include scenario context (feature-gated) |
| `rstest-bdd-harness-gpui`  | `macro_compile`              | trybuild compile-pass for GPUI fixtures (feature-gated)              |

These tests were moved out of `rstest-bdd` in this release to decouple the core
crate from Tokio and GPUI dev-dependencies, making it publishable to crates.io
without carrying those dependencies.

## First-party adapter dependency boundary

`rstest-bdd-harness` remains the owner of `HarnessAdapter`, `AttributePolicy`,
`ScenarioRunRequest`, and related base API types. The Tokio and GPUI adapter
crates re-export the subset of that API used by generated scenario code, so
downstream users of first-party adapters do not need to list
`rstest-bdd-harness` directly.

When updating macro code generation, keep this boundary intact:

- canonical Tokio harness and attribute policy paths should use the
  `rstest-bdd-harness-tokio` crate root for generated base API references;
- canonical GPUI harness and attribute policy paths should use the
  `rstest-bdd-harness-gpui` crate root for generated base API references;
- custom harnesses and custom attribute policies should continue to use the
  direct `rstest-bdd-harness` crate path and therefore require that dependency
  in the consuming crate.

## Fallback binary build in integration tests

`crates/cargo-bdd/tests/cli.rs` and `examples/todo-cli/tests/cli.rs` use a
two-phase strategy to locate test binaries, implemented by
`rstest_bdd_harness::binary_test_support::locate_or_build_binary`:

1. Try `assert_cmd::Command::cargo_bin("binary-name")`.
2. On failure, compute the expected debug binary path via
   `target_directory_for_manifest` and invoke `build_binary` if the binary is
   absent.

This pattern ensures tests run from a clean checkout without a separate
pre-build step in every CI job.

### `binary_test_support` API reference

```rust
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rstest_bdd_harness::binary_test_support::BinaryLocateError;

/// Returns the expected debug binary path for `binary_name` given a target
/// directory root. Pure computation: no I/O.
pub fn binary_path_in_target_dir(
    target_directory: &Path,
    binary_name: &str,
) -> PathBuf;

/// Resolves the workspace target directory by running `cargo metadata`.
/// Performs I/O: spawns a `cargo metadata` subprocess.
pub fn target_directory_for_manifest(
    manifest_path: &Path,
) -> Result<PathBuf, cargo_metadata::Error>;

/// Locates `binary_name` or builds it if absent; returns a ready `Command`.
/// On failure, returns [`BinaryLocateError`] so callers can match on kind
/// (metadata, spawn, build output, or missing binary).
pub fn locate_or_build_binary(
    manifest_path: &Path,
    workspace_root: &Path,
    binary_name: &str,
) -> Result<Command, BinaryLocateError>;

/// Builds `binary_name` via `cargo build --bin <name>` in `workspace_root`.
/// Returns the captured `Output`; returns `Err` only when the subprocess
/// cannot be spawned.
pub fn build_binary(
    workspace_root: &Path,
    binary_name: &str,
) -> std::io::Result<Output>;
```

**Usage example** (from `examples/todo-cli/tests/cli.rs`):

```rust
use assert_cmd::Command;

fn locate_or_build_todo_cli_cmd() -> Result<Command, Box<dyn std::error::Error>> {
    let root = workspace_root();
    locate_or_build_binary(&root.join("Cargo.toml"), &root, "todo-cli")
        .map(Command::from_std)
        .map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })
}
```

The module is `#[doc(hidden)]` and is not part of the public crates.io API. Do
not use it outside test helpers.

## Macro implementation: fixture classification and normalization

Fixture name normalization happens during macro expansion, before generated
wrappers ask the runtime context for fixture values. This keeps scenario-side
fixture registration and step-side fixture lookup on the same key scheme, so an
implicit parameter such as `_world` registers and resolves as `world`, while
`__world` resolves as `_world`.

The helper `normalize_param_name()` owns that rule. Use it whenever macro code
derives a fixture key from a Rust parameter name without an explicit override.
Keeping the rule centralized avoids one side of macro expansion stripping a
leading underscore while another side keeps it.

Step wrapper argument classification is handled by
`classify_by_placeholder_match()` in the macros crate. The function first
checks whether the argument maps to a step placeholder. If it does not, the
argument is classified as a fixture. For implicit fixture arguments, it records
the normalized fixture name so the generated wrapper asks for the same key that
scenario fixture registration produced.

Explicit `#[from(...)]` names are authoritative and bypass normalization. Use
that escape hatch when the intended fixture name starts with an underscore or
otherwise differs from the Rust parameter name. When the classifier must build
a new identifier for a normalized implicit fixture name, preserve the original
parameter span so diagnostics still point at the user-written parameter.

Generated wrappers must also submit typed fixture requirement metadata for
runtime missing-fixture diagnostics. Keep `Step::fixtures` as the public
name-only compatibility field, and publish `FixtureRequirement { name, ty }`
through the hidden `StepFixtureRequirements` inventory sidecar whenever macro
code knows the requested Rust type. Manual `step!` registrations without that
sidecar remain valid and report `<unknown>` as the requested fixture type.

## Shared policy crate (`rstest-bdd-policy`)

The workspace owns policy type definitions in `rstest-bdd-policy`.[^1] That
crate is the single source of truth for `RuntimeMode`, `TestAttributeHint`, and
their helper behavior inside this workspace.

`rstest-bdd` re-exports both shared policy types from the runtime API to
preserve its public contract.[^2]

```rust
pub use rstest_bdd_policy::{RuntimeMode, TestAttributeHint};
```

The re-export lives in
[`crates/rstest-bdd/src/execution/mod.rs`](../crates/rstest-bdd/src/execution/mod.rs),
so downstream users can continue to depend on
`rstest_bdd::execution::{RuntimeMode, TestAttributeHint}` without importing the
policy crate directly.

The macro layer imports both policy types directly from
`rstest_bdd_policy`;[^3] it does not define local duplicates of those enums.
Keep this boundary intact to avoid drift between macro parsing decisions and
runtime execution behaviour.

Add new shared policy types in `rstest-bdd-policy` when a type must be used by
both the runtime and macro crates. Keep type definitions local to the crate
that uses them when sharing is not needed.

Regression tests enforce this boundary:

- Runtime re-export assertions.[^4]
- Macro import assertions.[^5]

Shared first-party path constants also live in `rstest-bdd-policy` so macro
parsing, harness adapters, and documentation can agree on canonical policy
locations:

- `STD_HARNESS_PATH`
- `TOKIO_HARNESS_PATH`
- `GPUI_HARNESS_PATH`
- `DEFAULT_ATTRIBUTE_POLICY_PATH`
- `TOKIO_ATTRIBUTE_POLICY_PATH`
- `GPUI_ATTRIBUTE_POLICY_PATH`

Use `resolve_test_attribute_hint_for_policy_path()` when macro arguments name
an attribute-policy plugin path directly. Use
`resolve_test_attribute_hint_for_harness_path()` when `attributes = …` is
omitted and a first-party harness path should imply its default
`TestAttributeHint`. Both helpers deliberately require exact first-party paths;
unknown third-party paths and paths with extra components return `None`, so
external harnesses must still opt in with an explicit attribute policy.

The architectural rationale explains this decision and its consequences.[^6]

[^1]: ../crates/rstest-bdd-policy
[^2]: ../crates/rstest-bdd/src/execution/mod.rs
[^3]: ../crates/rstest-bdd-macros/src/macros/scenarios/macro_args/mod.rs
[^4]: ../crates/rstest-bdd/src/execution/tests.rs
[^5]: ../crates/rstest-bdd-macros/src/macros/scenarios/macro_args/tests.rs
[^6]: adr-004-policy-crate.md

## Internal test infrastructure

The async semantic behaviour tests use a shared support module at
`crates/rstest-bdd/tests/common/async_semantic_behaviour_support.rs`. Use the
helpers and types below when writing or extending semantic tests; do not access
`TEST_STATE` directly.

### Constants

Table: Async semantic behaviour support module constants

| Constant              | Value / purpose                                                                                                                                                    |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `FEATURE_PATH`        | Relative path (from `CARGO_MANIFEST_DIR`) to the async semantic behaviour feature file. Pass to `assert_feature_path_suffix` and as `ScenarioRef::feature_suffix`. |
| `SKIP_SCENARIO_NAME`  | Canonical name of the skip-propagation scenario. Use wherever a scenario name is required for that scenario.                                                       |
| `ERROR_SCENARIO_NAME` | Canonical name of the error-propagation scenario. Use wherever a scenario name is required for that scenario.                                                      |

### Parameter structs

Prefer struct-literal syntax at call sites so that each field is labelled.

#### `ScenarioRef<'a>`

Bundles the two string fields that identify a scenario in assertion helpers.

```rust
ScenarioRef {
    name:           ERROR_SCENARIO_NAME,
    feature_suffix: FEATURE_PATH,
}
```

Fields: `name: &'a str`, `feature_suffix: &'a str`.

#### `StepRef<'a>`

Bundles the four string fields that identify a step in failure-context
assertions.

```rust
StepRef {
    keyword:       "When",
    text:          "a step fails with an error",
    function_name: "step_that_fails",
    handler_error: "deliberate failure",
}
```

Fields: `keyword: &'a str`, `text: &'a str`, `function_name: &'a str`,
`handler_error: &'a str`.

#### `BypassedStepQuery<'a>` _(requires `diagnostics` feature)_

Bundles the four fields needed to look up a bypassed-step record in the
diagnostics registry dump.

Fields: `scenario_name: &'a str`, `scenario_line: u32`, `step_pattern: &'a str`,
`reason: &'a str`.

### Helper types

#### `SemanticValue(i32)`

Newtype wrapper for an integer fixture value. Used to verify that async step
handlers can return a value that is injected as a fixture into subsequent steps.

#### `CleanupProbe`

A zero-size marker struct whose `Drop` implementation increments the per-thread
`cleanup_drops` counter. Inject it as a fixture and call
`reset_cleanup_drops()` before the scenario under test, then assert
`cleanup_drops() == 1` after it completes (or after `catch_unwind` returns for
failure paths).

### Assertion helpers

#### `assert_feature_path_suffix(actual, expected_suffix)`

Asserts that `actual` ends with `expected_suffix` using `Path::ends_with`.
Panics with a descriptive message on mismatch.

#### `assert_handler_failure_context(message, ScenarioRef, StepRef)`

Normalizes `message` (converts backslashes to forward slashes, strips Unicode
directional marks) and asserts it matches a regex covering the step keyword,
step text, function name, handler error, feature path suffix, and scenario
name. Panics on regex compile failure or mismatch.

#### `assert_bypassed_step_recorded(BypassedStepQuery)` _(requires `diagnostics` feature)_

Dumps the diagnostics registry, parses it as JSON, and asserts that
`bypassed_steps` contains an entry matching all four fields of the query.
Panics if no matching entry is found.

### Event utilities

Table: Per-thread event log helpers for semantic behaviour tests

| Function                           | Purpose                                                                           |
| ---------------------------------- | --------------------------------------------------------------------------------- |
| `clear_events()`                   | Resets the per-thread event log. Call at the start of any test that reads events. |
| `push_event(event)`                | Appends a string to the per-thread event log. Call from within step handlers.     |
| `snapshot_events() -> Vec<String>` | Returns a clone of the current event log without clearing it.                     |

### Cleanup utilities

Table: Per-thread cleanup-probe drop counter helpers

| Function                   | Purpose                                                                          |
| -------------------------- | -------------------------------------------------------------------------------- |
| `reset_cleanup_drops()`    | Resets the per-thread drop counter to zero. Call before the scenario under test. |
| `cleanup_drops() -> usize` | Returns the number of times `CleanupProbe` has been dropped in this thread.      |

### Line-number utility

#### `scenario_line(scenario_name) -> u32`

Reads `FEATURE_PATH` relative to `CARGO_MANIFEST_DIR`, scans for a `Scenario:`
or `Scenario Outline:` heading whose name matches `scenario_name`, and returns
the 1-based line number. Panics if the scenario is not found. Use this instead
of hard-coded line numbers so that tests remain valid when the feature file is
edited.

### Thread-local state and test isolation

All mutable state (`events`, `cleanup_drops`) is held in a single
`thread_local! { RefCell<TestState> }`. State is per-thread and does not leak
between concurrently running threads. Any test that reads from or writes to
shared state must:

1. Call `clear_events()` and/or `reset_cleanup_drops()` at the start.
2. Be annotated with `#[serial]` to prevent interleaving with other
   tests on the same thread pool.

## Implementing a HarnessAdapter

### Overview

`HarnessAdapter::run` returns `HarnessResult<T>`, which is an alias for
`Result<T, HarnessError>`. Earlier versions returned `T` directly. The new
return type is a breaking change that makes harness initialization failures
explicit instead of forcing harness implementations to panic. This closes issue
`#443`.

Custom harnesses should thread harness-specific state through
`HarnessAdapter::Context`. Use `()` when no context is needed; otherwise,
choose a concrete context type, construct it inside `run`, and pass it to
`ScenarioRunRequest::run(context)`. Step functions request the harness context
with the reserved fixture key `rstest_bdd_harness_context`, for example
`#[from(rstest_bdd_harness_context)] context: &MyHarnessContext`.
`rstest_bdd_harness_tokio::TokioTestContext` shows the first-party Tokio
pattern: `TokioHarness` sets `type Context = TokioTestContext`, captures the
active runtime handle, and passes that per-scenario context to the runner.

### Return-type contract

`Ok(value)` carries the scenario outcome produced by the runner. If the
scenario itself returns a `Result`, that scenario-level result is nested inside
the `Ok` arm:

```rust
HarnessResult<Result<(), StepError>>
```

`Err(HarnessError::RuntimeBuildFailed(_))` is reserved for harness
infrastructure failures, such as failing to construct a Tokio runtime before
the scenario can run.

### Migration guidance

Existing `HarnessAdapter` implementations should make the following changes:

- Change the `run` return type to `HarnessResult<T>`.
- Wrap previously direct return values in `Ok(...)`.
- Replace `panic!` on runtime-build failure with
  `Err(HarnessError::RuntimeBuildFailed(err))`. Prefer mapping the build error
  and using `?` where possible:

  ```rust
  let runtime = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .map_err(HarnessError::RuntimeBuildFailed)?;
  ```

- For unit-context harnesses, switch from `request.run(())` to
  `request.run_without_context()`.

### Test-site guidance

Generated tests unwrap harness execution with:

```rust
unwrap_or_else(|err| panic!("harness failed to initialize scenario: {err}"))
```

Use the same pattern in hand-written tests instead of bare `.unwrap()`. This
keeps the concrete `HarnessError` visible in the panic message when a harness
cannot initialize its infrastructure.

### Staging: `copy_dir_tree` and missing destination parent chains

`trybuild_staging::copy_dir_tree(src, dst)` creates any missing parent
directories in the `dst` path before copying, so callers do not need to
pre-create the destination tree. For example, if `dst` is `tmp/a/b/c` and only
`tmp` exists, `copy_dir_tree` creates `tmp/a/b/c` and then copies the contents
of `src` into it.

To prevent accidental self-copies, `copy_dir_tree` resolves the canonical paths
of `src` and `dst` before copying and rejects any call where the resolved `src`
equals `dst`, `src` starts with `dst`, or `dst` starts with `src`. This check
is performed even when `dst` does not yet exist: the function walks up to the
nearest existing ancestor of `dst`, canonicalizes that ancestor, and re-appends
the missing tail components to obtain the resolved destination. This means that
paths such as `<src>/missing/../other` that traverse back into the source tree
through a not-yet-existing intermediate segment are still detected and rejected
with `io::ErrorKind::InvalidInput`.

### First-party policy path constants and resolver helpers

The `rstest-bdd-policy` crate exposes path constants and two resolver functions
that map a type-path to a `TestAttributeHint`.

#### Path constants

The following `&[&str]` constants identify the known first-party harness and
attribute-policy types:

Table: Path constants used for first-party policy and harness lookups

| Constant                        | Path segments                                          |
| ------------------------------- | ------------------------------------------------------ |
| `STD_HARNESS_PATH`              | `["rstest_bdd_harness", "StdHarness"]`                 |
| `TOKIO_HARNESS_PATH`            | `["rstest_bdd_harness_tokio", "TokioHarness"]`         |
| `GPUI_HARNESS_PATH`             | `["rstest_bdd_harness_gpui", "GpuiHarness"]`           |
| `DEFAULT_ATTRIBUTE_POLICY_PATH` | `["rstest_bdd_harness", "DefaultAttributePolicy"]`     |
| `TOKIO_ATTRIBUTE_POLICY_PATH`   | `["rstest_bdd_harness_tokio", "TokioAttributePolicy"]` |
| `GPUI_ATTRIBUTE_POLICY_PATH`    | `["rstest_bdd_harness_gpui", "GpuiAttributePolicy"]`   |

Use these constants wherever a first-party path must be compared or matched; do
not inline the string slices, as the constants are the canonical source of
truth and may be updated in future releases.

#### Resolver functions

Table: Resolver functions mapping a type path to a `TestAttributeHint`

| Function                                                                                   | Use                                                                                                                                                                                                                                          |
| ------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `resolve_test_attribute_hint_for_policy_path(path: &[&str]) -> Option<TestAttributeHint>`  | Returns the hint for a known first-party attribute-policy type path. Returns `None` for any path that is not an exact match for a known first-party policy path. Do not use this function for harness paths.                                 |
| `resolve_test_attribute_hint_for_harness_path(path: &[&str]) -> Option<TestAttributeHint>` | Returns the hint for a known first-party harness type path, delegating to the policy-path resolver for the corresponding attribute-policy type. Returns `None` for any path that is not an exact match for a known first-party harness path. |

Both functions require exact matches against first-party paths. Paths with
wrong prefixes, extra segments, or partial matches all return `None`. Use
`resolve_test_attribute_hint_for_harness_path` when the call site has a harness
type path; use `resolve_test_attribute_hint_for_policy_path` when it has an
attribute-policy type path.

### Third-party adapter crates

Third-party harness crates outside this workspace implement the same
`HarnessAdapter` and `AttributePolicy` contracts described here. The worked
example in the
[third-party harness adapter cookbook](users-guide.md#third-party-harness-adapter-cookbook)
shows the user-facing crate shape. Such crates should depend on
`rstest-bdd-harness` for the adapter contracts, keeping framework integration
decoupled from `rstest-bdd` and `rstest-bdd-macros`.

### Observability guidance

Harness implementations should emit a `tracing::error!` event before returning
`Err` from `HarnessAdapter::run`. Use structured fields so downstream test
runners and CI logs can filter by harness and scenario:

- `harness_type`: `std::any::type_name::<H>()` for the harness adapter type.
- `feature_path`: `request.metadata().feature_path()`.
- `scenario_name`: `request.metadata().scenario_name()`.
- `err`: the concrete `HarnessError`, formatted with `%err`.

Generated scenario delegation emits the same event and attaches scenario
context to the displayed error before panicking, so custom harnesses should use
matching field names for consistency.

### HarnessError extension

`HarnessError` is marked `#[non_exhaustive]`, so downstream code that matches
on it must include a `_` fallback arm. New variants may be added in minor
releases as more harness infrastructure failures become typed and inspectable.

## GpuiHarness panic-handling internals

The `rstest-bdd-harness-gpui` adapter wraps `gpui::run_test` in a thin
panic-aware envelope so that failing scenarios surface the originating feature
path, scenario name, and feature-file line in both the resumed panic payload
and observability sinks. The internals are intentionally private but worth
understanding when modifying the harness:

- `GpuiHarness::run_request_once` is the single entry point that drives
  `gpui::run_test`. It builds the per-scenario `TestAppContext`, constructs a
  `ContextCleanup` RAII guard, and wraps the runner closure in a
  `panic::catch_unwind(AssertUnwindSafe(..))` boundary. On the success path the
  result is stored in an output mutex; on the panic path the boxed `Any + Send`
  payload is rendered through `augmented_panic_message`, recorded via
  `record_and_write_panic_diagnostic`, leaked with `std::mem::forget` to
  neutralize any user-defined `Drop` that could double-panic, and finally
  re-raised as `Box<String>` through `panic::resume_unwind`. The caller injects
  the stderr writer (`AssertUnwindSafe<RefCell<W>>`) so I/O routing stays
  visible at the call site rather than hidden behind a no-argument default.
- `ContextCleanup` is an RAII guard that calls `finish_context` from
  its `Drop` impl. It is constructed immediately after the `TestAppContext` is
  built so the cleanup contract is honoured on both the success and the panic
  paths. `finish_context` drains the dispatcher with `run_until_parked`, calls
  `forbid_parking` on the executor, and quits the context, so parked timers or
  background work cannot leak into the next scenario.
- `augmented_panic_message` renders the boxed `Any + Send` payload via
  the workspace-shared `rstest_bdd::panic_message` downcast ladder (handles
  `&str`, `String`, common scalars, and falls back to an opaque
  `TypeId`-bearing description), then prepends the feature path, scenario name,
  and line drawn from `ScenarioMetadata`.
- `record_and_write_panic_diagnostic` calls `record_panic_event` to
  emit a `tracing::error!` record (with the harness, feature path, scenario
  name, scenario line, and rendered error as structured fields) and then writes
  the same message to the injected writer via `write_stderr_diagnostic_to`.
  Write errors are downgraded to `tracing::debug!` so an uncooperative stderr
  never escalates into a double panic.

Because the runtime mutates an `Rc`-backed `TestAppContext`, every test that
drives `GpuiHarness::run` from within the same process must be serialized under
`#[serial_test::serial]`. The harness exposes that constraint in its
module-level docs; both the in-module unit tests in
`crates/rstest-bdd-harness-gpui/src/gpui_harness/tests.rs` and the
feature-gated regression suite in
`crates/rstest-bdd-harness-gpui/tests/scenario_name_in_logs.rs` apply the
attribute to every `GpuiHarness::run`-driving test.


## Canonical diagnostic publish path

All LSP diagnostic publishing in `rstest-bdd-server` flows through the
canonical `publish_with` helper in
`crates/rstest-bdd-server/src/handlers/diagnostics/publish.rs`. It owns the
publish boundary exactly once: the client-socket guard, the
path-to-URI guard, `PublishDiagnosticsParams` construction, the
`textDocument/publishDiagnostics` notification, and failure logging.

- **Ownership:** the diagnostics handler layer owns the helper; it is private
  to the `diagnostics::publish` module.
- **Permitted call-sites:** the public per-file-kind functions
  (`publish_feature_diagnostics`, `publish_rust_diagnostics`, and any future
  variant). New diagnostic publishers must delegate to `publish_with` with a
  compute closure rather than re-implementing the guards or notify call.
- **Composition rules:** the compute closure returns
  `Option<Vec<Diagnostic>>` — `None` skips publishing entirely (used when a
  feature file has no index, preserving previously published diagnostics),
  while `Some(vec![])` still publishes so stale diagnostics are cleared.
  `prepare_publish` separates parameter construction from the notify side
  effect so tests can pin payloads without a client socket.

The published payloads for representative feature and Rust files are pinned
by `insta` snapshots, and the publish invariants (count preserved, empty
vector still published) by a property test, both in
`handlers/diagnostics/publish.rs`.

## Registry lookup usage-marking invariant

Every public step-lookup function in `crates/rstest-bdd/src/registry/`
(`lookup_step`, `find_step`, `lookup_step_async`, `find_step_async`,
`lookup_step_async_with_mode`, `find_step_async_with_mode`, and
`find_step_with_metadata`) funnels through the canonical private helper
`mark_and_project` in `registry/mod.rs`. The helper performs the `mark_used`
bookkeeping exactly once and applies the caller's projection to the resolved
`Step`.

- **Invariant:** every lookup that returns `Some` marks exactly the resolved
  step as used (feeding the unused-step diagnostics behind `cargo bdd`); a
  lookup that returns `None` marks nothing.
- **Permitted call-sites:** the public lookup wrappers in `registry/mod.rs`
  and `registry/async_lookup.rs`. New lookup variants must resolve a step
  (via `resolve_exact_step` / `resolve_step`) and pass it through
  `mark_and_project`; calling `mark_used` directly from a lookup is a bug.
- The invariant is pinned across all variants by the property suite in
  `crates/rstest-bdd/tests/registry_mark_used_props.rs`. A `kani` harness was
  considered and omitted: the registry is backed by link-time `inventory`
  registration and a lazily built hash map, which a bounded harness cannot
  model cheaply, and the property suite already exercises every variant
  against hit and miss lookups.

## Planned internal APIs and tooling (ADR-010 to ADR-012)

Three accepted-as-`Proposed` ADRs schedule internal-API and build-tooling
changes that contributors will encounter as the v0.6.1 and v0.7.0 work lands.
They are summarised here so the decisions are discoverable from the developer
guide; the ADRs remain the authoritative source, and the planning rationale
lives in
[`docs/execplans/adopt-v0-6-0-beta2-feedback.md`](execplans/adopt-v0-6-0-beta2-feedback.md).

### Scenario-state helpers and per-scenario cleanup (ADR-011)

[ADR-011](adr-011-first-party-scenario-state-and-cleanup.md) introduces a
first-party replacement for the hand-rolled thread-local `RefCell` plus `Drop`
cleanup guard that stateful GPUI scenarios use today:

- A generic `ScenarioStore<T>` core lives in `rstest-bdd`, exposing
  `set`/`with`/`with_mut`/`take`/`reset` and wrapping the two-sided reset
  protocol. It is named to avoid colliding with the already-shipped
  `pub trait ScenarioState` and `pub struct Slot<T>` in
  `crates/rstest-bdd/src/state.rs`; it composes with `Slot<T>` rather than
  shadowing it.
- A `GpuiScenarioStore` specialisation plus a cleanup-guard fixture macro ship
  from `rstest-bdd-harness-gpui`. The layering is acyclic: the harness crate
  already depends on `rstest-bdd`, and the core never imports the harness.
- The cleanup-ordering contract (reset before assignment; `Drop` cleanup on
  success, assertion failure, and skip) is fixed by the ADR and must be covered
  by unit, property-based (`proptest`), and `serial_test`-guarded
  thread-isolation tests — see the ADR's _Testing strategy_.

Tracked by roadmap items 11.1.3 and 11.1.4 (pulled forward to v0.6.0 final);
design coverage is in `rstest-bdd-design.md` §2.7.6.4.

### Guard-based `StepContext` borrowing and `FixtureBorrowError` (ADR-012)

[ADR-012](adr-012-guard-based-stepcontext-borrowing.md) records the v0.7.0
redesign of `StepContext` borrowing as a committed direction, not an ambition.
Contributors touching the borrow machinery should expect:

- `StepContext::borrow_mut(&mut self, …)` is replaced by interior borrowing
  that returns `FixtureRefMut` guards, so two guards for _distinct_ keys can
  coexist (for example `&mut TestAppContext` alongside `&mut World`) while two
  guards for the _same_ key fail. This removes the `E0499`/`E0502` constraint
  that forces today's thread-local workaround.
- Borrow APIs return `Result` carrying a structured `FixtureBorrowError`
  (`MissingFixture`, `TypeMismatch`, `AlreadyBorrowed`). Roadmap item 11.1.1
  introduces an early version of this error surface in v0.6.1; v0.7.0 completes
  it.
- `FixtureRefMut` exposes a stable, opaque accessor API (12.1.2), and a
  first-class world lifecycle contract (12.1.3) supersedes the thread-local
  reset protocol and the v0.6.1 `ScenarioStore<T>` helper. The ADR carries the
  v0.6→v0.7 migration mapping.
- Borrow-state invariants are the highest-risk part of the surface and must be
  covered by generated-wrapper, property-based (`proptest`), and lifecycle
  tests — see the ADR's _Testing strategy_.

Tracked by roadmap items 12.1.1–12.1.3; design coverage is in
`rstest-bdd-design.md` §2.7.6.5.

### Feature-file rebuild invalidation (ADR-010)

[ADR-010](adr-010-feature-file-change-detection.md) closes a build-tooling
foot-gun: `#[scenario(path = …)]` and `scenarios!` read `.feature` files with
`std::fs` at macro-expansion time, so Cargo never sees them as inputs and a
`.feature`-only edit does not trigger a rebuild. The decision:

- For single-file `#[scenario]` binding, prefer emitting a **relative-path**
  `include_str!` so rustc registers the file in dep-info automatically. An
  absolute `CARGO_MANIFEST_DIR`-rooted path is **rejected** because it breaks
  reproducible builds (Nix sandboxes, `sccache`, Windows/POSIX separators).
- For `scenarios!` directory-glob binding, prefer a build-script helper
  emitting `cargo::rerun-if-changed` for the features directory and each
  discovered file (the `theoremc` pattern), which embeds nothing in the
  artefact.
- The unstable `proc_macro::tracked_path` API is the long-term answer, usable
  behind a `nightly` feature gate once stabilised.
- Invalidation must be a _tested contract_: a portability-aware rebuild
  regression test, a `trybuild` compile-time test for the emitted binding, and
  redacted `insta` snapshots for any touched diagnostic — see the ADR's
  _Testing strategy_. This is distinct from the OUT_DIR AST _caching_
  performance idea in `rstest-bdd-design.md` §3.2.2.

Tracked by roadmap item 11.3.1 (pulled forward to v0.6.0 final); design
coverage is in `rstest-bdd-design.md` §2.7.6.6. Until it lands,
`v0-6-0-migration-guide.md` carries a caveat that `.feature`-only edits do not
trigger a rebuild.

## Language-server handler conventions

### Canonical extension predicate: `has_extension`

`rstest_bdd_server::handlers::util::has_extension(path, ext)` is the single
canonical predicate for testing a path's file extension in handler code. It
compares the path's final extension against `ext` (supplied without a leading
dot) using ASCII case-insensitive equality, and returns `false` for paths
with no extension.

- **Ownership:** the helper lives in `handlers/util.rs` and is owned by the
  language-server handler layer.
- **Permitted call-sites:** any LSP handler that needs to distinguish `.rs`
  from `.feature` paths (definition, implementation, text-document save, and
  future handlers). Code outside the server crate should not depend on it.
- **Composition rules:** call it directly with a literal extension
  (`has_extension(&path, "rs")`); do not wrap it in per-handler `is_*_file`
  aliases, because such wrappers reintroduce the duplication this helper
  removed. If a handler needs a new file kind, pass the new literal at the
  call-site.

Invariants (ASCII-case insensitivity, rejection of differing extensions, and
behaviour for missing, repeated, and trailing dots) are pinned by the
property suite in `crates/rstest-bdd-server/tests/has_extension_props.rs`.
