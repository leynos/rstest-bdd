# Developer guide

## Workspace dependency policy

Keep workspace-local development and crates.io publication on the same
manifest surface by declaring shared dependencies in the root
`[workspace.dependencies]` table. First-party crates must use both `version`
and `path` there, then consume the dependency with `.workspace = true` from
member manifests. The `path` keeps local builds on the current checkout after a
version has been published, while the `version` gives Cargo the crates.io
requirement it needs when packaging a crate.

Do not restore root-level `[patch.crates-io]` entries for normal development.
Patches make local resolution differ from publish-time resolution and can hide
registry-only failures. If a temporary patch is required for a one-off
diagnostic, remove it before committing or teach the publish-check automation
to strip it explicitly.

The GPUI test shim follows the same pattern. The workspace dependency for
`gpui` points at `vendor/gpui` with a matching crates.io version, so local tests
use the stable-compatible shim. The publish-check GPUI package validator strips
that local path when it generates the standalone harness manifest, so
`rstest-bdd-harness-gpui` is still checked against the upstream `gpui`
dependency surface before publication.

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
- `.cargo/nextest.toml` raises the `slow-timeout` for `binary(macro_compile)`
  on Windows to 300 s as a local-development safety net. This does not fix the
  deadlock; it only delays termination to allow the build to complete on fast
  machines.
- Do not add `macro_compile`-style tests (tests that spawn `cargo` via
  `trybuild` or `cargo_metadata`) to nextest-managed binaries intended to run
  on Windows.

## Test organization: harness-owned integration tests

Tokio and GPUI harness integration tests are co-located with their respective
harness crates:

| Crate                      | Test binary       | What it tests                                           |
| -------------------------- | ----------------- | ------------------------------------------------------- |
| `rstest-bdd-harness-tokio` | `scenario_macros` | `#[scenario]` + Tokio adapter                           |
| `rstest-bdd-harness-tokio` | `macro_compile`   | trybuild compile-pass/fail for Tokio fixtures           |
| `rstest-bdd-harness-gpui`  | `scenario_macros` | `#[scenario]` + GPUI adapter (feature-gated)            |
| `rstest-bdd-harness-gpui`  | `macro_compile`   | trybuild compile-pass for GPUI fixtures (feature-gated) |

These tests were moved out of `rstest-bdd` in this release to decouple the core
crate from Tokio and GPUI dev-dependencies, making it publishable to crates.io
without carrying those dependencies.

## First-party adapter dependency boundary

`rstest-bdd-harness` remains the owner of `HarnessAdapter`,
`AttributePolicy`, `ScenarioRunRequest`, and related base API types. The Tokio
and GPUI adapter crates re-export the subset of that API used by generated
scenario code, so downstream users of first-party adapters do not need to list
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

The module is `#[doc(hidden)]` and is not part of the public crates.io API.
Do not use it outside test helpers.

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

## Shared policy crate (`rstest-bdd-policy`)

The workspace owns policy type definitions in
`rstest-bdd-policy`.[^1] That crate is the single source of truth for
`RuntimeMode`, `TestAttributeHint`, and their helper behavior inside this
workspace.

`rstest-bdd` re-exports both shared policy types from the runtime API to preserve
its public contract.[^2]

```rust
pub use rstest_bdd_policy::{RuntimeMode, TestAttributeHint};
```

The re-export lives in
[`crates/rstest-bdd/src/execution/mod.rs`](../crates/rstest-bdd/src/execution/mod.rs),
so downstream users can continue to depend on
`rstest_bdd::execution::{RuntimeMode, TestAttributeHint}` without importing the
policy crate directly.

The macro layer imports both policy types directly from
`rstest_bdd_policy`;[^3] it does not define local duplicates of those enums. Keep
this boundary intact to avoid drift between macro parsing decisions and runtime
execution behaviour.

Add new shared policy types in `rstest-bdd-policy` when a type must be used by
both the runtime and macro crates. Keep type definitions local to the crate that
uses them when sharing is not needed.

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
- `GPUI_ATTRIBUTE_POLICY_PATH`

Use `resolve_test_attribute_hint_for_policy_path()` when macro arguments name
an attribute-policy plugin path directly. Use
`resolve_test_attribute_hint_for_harness_path()` when `attributes = ...` is
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

Fields: `scenario_name: &'a str`, `scenario_line: u32`,
`step_pattern: &'a str`, `reason: &'a str`.

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

| Function                           | Purpose                                                                           |
| ---------------------------------- | --------------------------------------------------------------------------------- |
| `clear_events()`                   | Resets the per-thread event log. Call at the start of any test that reads events. |
| `push_event(event)`                | Appends a string to the per-thread event log. Call from within step handlers.     |
| `snapshot_events() -> Vec<String>` | Returns a clone of the current event log without clearing it.                     |

### Cleanup utilities

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

### Third-party adapter crates

Third-party harness crates outside this workspace implement the same
`HarnessAdapter` and `AttributePolicy` contracts described here. The worked
example in the [third-party harness adapter cookbook](users-guide.md#third-party-harness-adapter-cookbook)
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
