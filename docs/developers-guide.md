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

## Rust documentation policy and gates

The workspace denies missing documentation on Rust items and crate roots. It
also denies broken or private intra-doc links, bare URLs, invalid HTML tags,
invalid code-block attributes, and unescaped backticks through the
`[workspace.lints.rust]` and `[workspace.lints.rustdoc]` tables in
`Cargo.toml`. Workspace crates inherit these lints with
`lints.workspace = true`; keep new modules and public APIs documented instead
of suppressing a lint.

`make lint` builds the workspace documentation after Clippy with:

```makefile
RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" $(CARGO) doc --workspace --no-deps
```

The Makefile exposes the flag value as
`RUSTDOC_FLAGS ?= --cfg docsrs -D warnings`, so callers may override it for
diagnostics. The committed default enables `docsrs`-conditional documentation
and promotes every Rustdoc warning to an error. `--workspace` checks every
member crate, while `--no-deps` keeps the gate focused on documentation owned
by this repository.

`make test` separately compiles and runs documentation examples for every
workspace crate with all features enabled:

```makefile
RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) test --doc --workspace --all-features $(BUILD_JOBS)
```

Keep the documentation build and all-feature doctest commands distinct: the
former validates generated documentation and links under the docs.rs
configuration, while the latter verifies that executable examples compile and
behave as documented.

## nextest configuration (`.config/nextest.toml`)

cargo-nextest reads its configuration from `.config/nextest.toml` at the
workspace root; this is the only nextest configuration file the runner loads.
The file sets the timeout policy for the test suite:

- The default profile kills any test that runs past a 60 s `slow-timeout`
  (`terminate-after = 1`, 5 s grace period) and applies a 20 m `global-timeout`
  to the whole run. This allows the cargo-spawning group to run its bounded
  tests one at a time without exhausting the whole-suite budget.
- A `[[profile.default.overrides]]` entry raises the `slow-timeout` to 180 s
  for `cargo-bdd::cli`, whose smoke tests spawn `cargo` to build fixture crates
  and can legitimately exceed 60 s on cold caches.
- A second override raises the `slow-timeout` further, to 300 s, for the
  trybuild-based compile-test binaries:
  `rstest-bdd-harness-tokio::macro_compile`,
  `rstest-bdd-harness-gpui::macro_compile`, and `rstest-bdd::trybuild_macros`.
  These tests invoke `cargo build` against a large dependency tree, so a cold
  cache (or CPU contention when several compile tests run concurrently) can
  push a single test well past the default limit even though nothing is wrong.
- Both overrides also place their binaries in a `cargo-spawning` test group
  (`max-threads = 1`), so `cargo-bdd::cli` and the three trybuild binaries run
  one at a time instead of contending for CPU with concurrent `cargo` builds.
- A `long` profile (`--profile long`) relaxes the limits further (180 s
  `slow-timeout`, 30 m `global-timeout`) for deliberately slow local runs.

When adding a test binary that shells out to `cargo`, extend the relevant
override's `filter` expression rather than raising the default `slow-timeout`:
the tight default is what surfaces genuinely hung tests quickly.

## `#[serial]`, `#[file_serial]`, and nextest test-groups

Stateful GPUI scenarios keep `#[serial]` even though this repository's
`make test` target runs under cargo-nextest. The annotation is required for
`cargo test`, where all tests in one binary share a process and the
`serial_test` mutex prevents stateful scenarios from interleaving on the same
test thread. Under nextest each test runs in its own process, so `#[serial]` is
redundant-but-harmless and exists for runner compatibility rather than nextest
scheduling.

Do not add a live test-group to `.config/nextest.toml` for the current stateful
GPUI suite. The repository has no shared cross-process resource that needs a
nextest-level mutex; process-per-test isolation is enough for the local
thread-local state. Add a test-group only when a new test resource is genuinely
global across processes or binaries, and keep the filter as narrow as the
resource allows.

Treat `#[file_serial]` as adopter guidance, not a repository dependency. It is
useful when a consuming workspace wants the exclusion to live on Rust tests
rather than in nextest configuration, but it requires `serial_test`'s
`file_locks` feature and does not mutually exclude `#[serial]` tests because
the two attributes use different lock mechanisms. If a future repository test
does need cross-process exclusion, choose one convention for that resource and
document the choice beside the test or nextest override.

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
- `step_macros_compile` (`crates/rstest-bdd/tests/trybuild_macros.rs`) guards
  its early return with `cfg!(windows) && env::var_os("NEXTEST_RUN_ID")`, so it
  only skips its trybuild and Clippy UI fixtures under nextest on Windows,
  where this deadlock applies. On Linux and macOS the fixtures run under
  nextest like any other test.
- `.config/nextest.toml` raises the `slow-timeout` for the trybuild
  compile-test binaries (including both `macro_compile` binaries and
  `rstest-bdd::trybuild_macros`) to 300 s as a local-development safety net.
  This does not fix the deadlock; it only delays termination to allow the build
  to complete on fast machines.
- `.config/nextest.toml` also places the `cargo-bdd::cli` and trybuild
  binaries in a `cargo-spawning` test group with `max-threads = 1`, so these
  cargo-spawning tests run one at a time rather than contending for the
  package-cache lock and build parallelism.
- Do not add `macro_compile`-style tests (tests that spawn `cargo` via
  `trybuild` or `cargo_metadata`) to nextest-managed binaries intended to run
  on Windows.

## Users-guide link validation (`scripts/check_users_guide_links.py`)

`docs/users-guide.md` is vendored into consumer projects, so its
cross-references to other documents in this repository use absolute GitHub URLs
(collected as reference-style definitions at the bottom of the file) rather
than relative paths. `scripts/check_users_guide_links.py`, run automatically by
`make lint`, keeps those URLs honest:

- Every repository reference must start with the canonical base URL recorded
  in the script's `BASE_URL` constant (currently
  `https://github.com/leynos/rstest-bdd/blob/main/docs/`). If the repository
  moves, the default branch is renamed, or the documents relocate, update that
  one constant and the reference block; the check pinpoints every definition
  that disagrees.
- Each link must resolve to an existing file under `docs/`, and any `#`
  fragment must match a heading anchor in the target document (the script
  derives anchors with GitHub's slug rules). Prefer heading fragments over
  `#L<n>` line anchors, which silently break on reflows.
- The check also fails if the guide contains no repository references at
  all, so a reformat cannot silently defang it.

Non-repository URLs (for example docs.rs links) are ignored. Unit tests live in
`scripts/tests/test_check_users_guide_links.py` and run with the Python suite in
`make test`. Issue #537 tracks generating the reference block from `BASE_URL`
so the base lives in exactly one place.

### Running the checker

Run it directly during development:

```bash
python3 scripts/check_users_guide_links.py [--root PATH]
```

Without `--root`, the checker derives the repository root relative to the
script itself, so it validates this checkout wherever it is invoked from.
`--root` exists to support the temporary-tree CLI integration tests and to
validate another checkout locally. `make lint` runs the checker using its
normal default root, so the gate always covers the working repository.

### Test split

- Unit and Hypothesis property tests live in
  `scripts/tests/test_check_users_guide_links.py`. Hypothesis exercises the
  slug-generation invariants (anchors stay lowercase, contain no spaces, use
  only word characters and hyphens, and are idempotent) and fenced-code heading
  handling, where generated headings expose parser edge cases that
  example-based cases miss.
- Cuprum subprocess/CLI integration tests live in
  `scripts/tests/test_check_users_guide_links_cli.py`. They verify
  process-level behaviour: exit status, stderr content, `--help` output,
  explicit `--root` against a temporary tree, and default-root execution.

The test-tooling rule for this script, and for scripts like it: use Hypothesis
for property and invariant coverage wherever generated inputs expose parser or
normalization edge cases, and use Cuprum for subprocess CLI behaviour rather
than Python's `subprocess` module.

### Intentional scope

The validator is deliberately retained and deliberately narrow. It is retained
because `docs/users-guide.md` is vendored into consumer projects, so a broken
absolute link can ship downstream undetected; manual review does not give
deterministic drift detection. Its scope is limited to repository-reference
link definitions in `docs/users-guide.md`, rather than expanding to every
documentation cross-reference in the repository. See
[ADR-014](adr-014-retain-users-guide-link-validator.md) for the decision record.

## GPUI mapping-table validation (`scripts/check_gpui_mapping_table.py`)

The vendored-to-published GPUI mapping table is duplicated in
`docs/users-guide.md` and `docs/rstest-bdd-design.md`. Update both copies
together whenever a GPUI test API shape changes. `make lint` runs
`scripts/check_gpui_mapping_table.py`, which anchors each table by its
surrounding heading and compares the four data rows after whitespace
normalization, so doc-vs-doc drift fails locally and in Continuous Integration
(CI).

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
repository with `git show <commit>:crates/gpui/src/app/test_context.rs`. Unit
tests for the table checker live in
`scripts/tests/test_check_gpui_mapping_table.py`.

## `#[serial]`/nextest matrix validation (`scripts/check_serial_nextest_matrix.py`)

The runner matrix for `#[serial]`, cargo-nextest, `#[file_serial]`, and nextest
test-groups is duplicated in `docs/users-guide.md` and
`docs/rstest-bdd-design.md`. Update both copies together whenever the
repository changes its runner guidance. `make lint` runs
`scripts/check_serial_nextest_matrix.py`, which anchors both copies by the
"Test-runner parallelism and scenario state" heading and compares the two data
rows after whitespace normalization.

The check only enforces doc-vs-doc parity. It does not validate nextest or
`serial_test` behaviour directly; verify those facts from the upstream nextest
and docs.rs references before changing the matrix. Unit tests for the checker
live in `scripts/tests/test_check_serial_nextest_matrix.py`.

Link-checker and table-checker tests run with the Python suite in `make test`.
Issue #537 tracks generating the users-guide reference block from `BASE_URL` so
the base lives in exactly one place.

## Workflow pins and Dependabot

Dependabot owns the upgrade of GitHub Actions and reusable workflows,
including calls into `leynos/shared-actions`. Contract tests that assert a
caller's exact commit SHA create a lockstep dependency: every time Dependabot
opens a bump PR, the test fails until a human edits the pinned constant to
match. That defeats the purpose of automated dependency updates and turns a
routine bump into a manual chore.

Contract tests may still verify the _shape_ of a reusable-workflow caller.
They must not verify the specific SHA value.

- Do assert the workflow references the correct reusable workflow path.
- Do assert the ref is pinned to a full 40-character commit SHA, not a
  mutable branch such as `main` or `rolling`.
- Do assert the expected `on:` triggers, least-privilege `permissions:`, and
  the inputs the caller relies on.
- Do not hard-code the current SHA value as an expected string. Match it with
  a pattern instead.
- Do not fail a test purely because Dependabot bumped the pinned SHA.

```python
import re

SHA_RE = re.compile(r"^[0-9a-f]{40}$")

def test_uses_pinned_full_sha(caller_step):
    ref = caller_step["uses"].split("@")[-1]
    assert SHA_RE.match(ref), f"expected a 40-hex commit SHA, got {ref!r}"
```

If a workflow's behaviour genuinely depends on a feature only present from a
particular commit onwards, express that as a comment or a changelog note, not
as a test assertion on the SHA string.

## Spelling policy

`make spelling` enforces en-GB-oxendict spelling over tracked text with the
pinned Typos release. `make spellcheck` remains an alias for existing tooling,
and `make markdownlint` depends on the same gate, so prose checks cannot bypass
the repository-wide spelling policy.

The checked-in `typos.toml` is generated from the shared dictionary and the
repository overlay in `typos.local.toml`. Do not edit generated entries by
hand. Run `make spelling-config-write` after changing the overlay or after the
shared dictionary is updated, and use `make spelling-config` to verify that the
checked-in result is current. The builder keeps its downloaded shared base in
untracked cache files and refreshes the local copy only when the published
source is newer.

Repository exceptions must protect machine interfaces, formal upstream names,
foreign-language catalogues, or exact serialized fixtures. Use the narrowest
anchored pattern or path exclusion possible and explain why it is required. Do
not add broad word-level exceptions for prose. The consumer phrase checker also
rejects punctuation-sensitive shared corrections that single-token spelling
scans cannot enforce reliably.

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
| `rstest-bdd-harness-tokio` | `harness_led_defaults`       | harness-led default inference and runtime error paths                |
| `rstest-bdd-harness-tokio` | `macro_compile`              | trybuild compile-pass/fail for Tokio fixtures                        |
| `rstest-bdd-harness-gpui`  | `harness_behaviour`          | GPUI harness adapter execution semantics (feature-gated)             |
| `rstest-bdd-harness-gpui`  | `attribute_policy_behaviour` | GPUI attribute policy output (feature-gated)                         |
| `rstest-bdd-harness-gpui`  | `scenario_macros`            | `#[scenario]` + GPUI adapter (feature-gated)                         |
| `rstest-bdd-harness-gpui`  | `harness_led_defaults`       | harness-led default inference and runtime error paths                |
| `rstest-bdd-harness-gpui`  | `stateful_window`            | durable GPUI handles + visual context reconstruction (feature-gated) |
| `rstest-bdd-harness-gpui`  | `scenario_name_in_logs`      | GPUI step-panic diagnostics include scenario context (feature-gated) |
| `rstest-bdd-harness-gpui`  | `macro_compile`              | trybuild compile-pass for GPUI fixtures (feature-gated)              |

These tests were moved out of `rstest-bdd` in this release to decouple the core
crate from Tokio and GPUI dev-dependencies, making it publishable to crates.io
without carrying those dependencies.

The GPUI `harness_led_defaults` happy-path scenario is gated by
`native-gpui-tests` and uses `#[serial]`, matching the native GPUI runtime
suite. Its failing-harness error-path scenario does not require the native GPUI
runtime and runs without the feature gate.

The `Failing harness initialization propagates` scenario's panic assertion is
de-duplicated for harness integration coverage: the shared scenario assertion
macro lives under `crates/rstest-bdd-harness/tests/support/` and is included
from both `harness_led_defaults.rs` test binaries (Tokio and GPUI) to keep
runtime-error-path assertions aligned. The guard step remains local to each
test binary, so the `#[scenario]` macro can discover it from the test source.

`rstest-bdd-harness` exposes `FailingHarness` from its crate root when its
`testing` feature is enabled. This dependency-facing test API always returns a
synthetic `HarnessError::RuntimeBuildFailed`, allowing adapter crates to share
the generated scenario error-path assertions without duplicating a failing
adapter. `rstest-bdd-harness`'s own test targets (for example
`tests/harness_behaviour.rs`) reach it through a self dev-dependency in its
`Cargo.toml`:

```toml
[dev-dependencies]
rstest-bdd-harness = { path = ".", features = ["testing"] }
```

This keeps `FailingHarness` defined once, with no local duplicate in the test
binary, and works without requiring `--all-features`. Downstream crates enable
it only for tests:

```toml
[dev-dependencies]
rstest-bdd-harness = { workspace = true, features = ["testing"] }
```

The core macro trybuild test scopes `RUST_BACKTRACE` removal with
`temp_env::with_var_unset`. Trybuild compares compiler diagnostics verbatim,
and CI's inherited backtrace setting otherwise adds platform-specific output.
Keep `temp-env` available as a dev-dependency when building this test target;
direct environment mutation is unsafe under Rust 2024 and would violate the
workspace's unsafe-code policy.

### Fallible GPUI test boundaries

The `rstest-bdd-macros` scenario code generator owns the private
`adapt_fallible_gpui_boundary` helper. Its scope is limited to fallible
functions generated with the first-party GPUI test policy, for both regular
scenarios and outlines. It changes the generated signature to return `()` and
consumes the scenario result, panicking with a fixed message on `Err`. Unit
scenarios and std or Tokio boundaries must bypass the helper unchanged.

Scenario generators call `generate_test_attrs_with_boundary`, which owns
attribute-policy resolution and returns both the emitted attributes and whether
GPUI owns the outer test boundary. Keep policy resolution inside this helper so
regular and outline generation reuse one result rather than allocating path
segments twice. Callers outside scenario code generation should use the policy
interfaces instead of composing this private result.

The private `finalize_scenario_signature` helper is owned by regular and
outline scenario generation. It composes trait assertions, resolved test
attributes, GPUI boundary adaptation, and underscore-expect tokens while
leaving emission order under caller control. It clones signatures lazily, only
when adaptation requires mutation. Other code-generation paths must not reuse
it unless they share this complete boundary contract.

Compile-pass coverage belongs to the GPUI harness crate because it supplies the
`#[gpui::test]` boundary. Keep the synchronous, asynchronous, and outline
non-`Debug` error cases in the `tests/fixtures_macros` directory of
`rstest-bdd-harness-gpui`: `scenario_fallible_non_debug.rs` and
`scenario_fallible_outline_non_debug.rs`. Register them with that crate's
`macro_compile` test. Runtime coverage in `tests/scenario_macros.rs` must also
execute an `Err` result and assert the fixed boundary panic.

The boundary has a closed, branch-oriented state space: GPUI ownership,
fallible or unit return, synchronous or asynchronous execution, and regular or
outline generation, with direct, explicit-policy, and inferred-harness GPUI
selection. Maintain finite parameterized cases for each decision branch, plus
the compile and runtime fixtures above. Property testing and a full Cartesian
product are unnecessary because repeated combinations reach the same branches,
while randomized `syn` syntax adds no semantic states or useful shrinking
oracle.

### Bulk-migration cookbook reference suite

The user guide's "Bulk-migration cookbook" subsection is backed by a
harness-agnostic reference in the core `rstest-bdd` crate:
`tests/common/bulk_migration_steps.rs` is one shared durable-state step library
that `tests/bulk_migration_cookbook_a.rs` and `_b.rs` each include through a
`#[path]` module and bind to their own feature file under
`tests/features/bulk_migration/`. The binding files carry no step definitions;
that "zero steps in the binding" property is the reuse proof and must be
preserved. A trybuild compile-pass mirror,
`tests/fixtures_macros/scenario_bulk_migration_cookbook.rs`, compile-checks the
same shape and is registered in `run_passing_macro_tests`
(`tests/trybuild_macros.rs`). `step_macros_compile` runs this fixture under
nextest on Linux and macOS; it is skipped only under nextest on Windows, where
the Job Object capture-pipe deadlock applies (see "nextest on Windows: trybuild
deadlock" above), and must instead be validated with plain `cargo test` (or
`cargo llvm-cov test` for coverage).

Doc↔suite parity for this cookbook is guarded by prose, not a checker (the
subsection states "if a snippet drifts, the suite wins"), matching the
third-party harness cookbook convention rather than the doc↔doc parity scripts.
The GPUI durable-handle specialization of the pattern is documented in prose
and remains proven by the `stateful_window` binary above; keep the cookbook's
GPUI snippets bridged to published `gpui 0.2.2` through the
vendored-to-published mapping table, and keep edits near that table and the
`#[serial]`/nextest matrix green under `check_gpui_mapping_table.py` and
`check_serial_nextest_matrix.py`.

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

Step wrapper argument classification enters through
`classify_fixture_or_step()`, the terminal classifier in the pipeline. It
strips any `#[from(...)]` attribute in place — rejecting a duplicate `#[from]`
and the `#[from = ...]` name-value form — and threads a `ClassificationContext`
(the mutable `extracted` results and remaining `placeholders` accumulators) into
`classify_by_placeholder_match()`. That helper first checks whether the
argument maps to a step placeholder. If it does not, the argument is classified
as a fixture. For implicit fixture arguments, it records the normalized fixture
name, so the generated wrapper asks for the same key that scenario fixture
registration produced.

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

### Generated-wrapper Tokio bridge

`rstest-bdd` owns the hidden `__rstest_bdd_tokio` re-export of its Tokio
runtime dependency. Generated async step wrappers in `rstest-bdd-macros` are
its only permitted call-sites: they use the bridge to detect an active runtime,
build a current-thread fallback runtime, and create a `LocalSet`. Downstream
step code must not reference the bridge directly or depend on a particular
Tokio crate name.

The bridge composes only from macro-generated wrappers through the resolved
`rstest_bdd` crate path; it is not a general runtime facade and must not be
re-exported by harnesses or custom adapters. Keeping Tokio as an `rstest-bdd`
runtime dependency gives generated code one stable, hygienic source-level path
regardless of how a downstream crate names, re-exports, or otherwise obtains
Tokio. Although marked `#[doc(hidden)]`, changing or removing this bridge is a
breaking change for existing async-step macro expansions.

## Shared policy crate (`rstest-bdd-policy`)

The workspace owns policy type definitions in `rstest-bdd-policy`.[^1] That
crate is the single source of truth for `RuntimeMode`, `TestAttributeHint`, and
their helper behaviour inside this workspace.

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
[^5]: ../crates/rstest-bdd-macros/src/macros/scenarios/macro_args/tests/mod.rs
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

Use the same pattern in handwritten tests instead of bare `.unwrap()`. This
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

All Language Server Protocol (LSP) diagnostic publishing in `rstest-bdd-server`
flows through the canonical `publish_with` helper in
`crates/rstest-bdd-server/src/handlers/diagnostics/publish.rs`. It owns the
publish boundary exactly once: the client-socket guard, the path-to-URI guard,
`PublishDiagnosticsParams` construction, the `textDocument/publishDiagnostics`
notification, and failure logging.

- **Ownership:** the diagnostics handler layer owns the helper; it is private
  to the `diagnostics::publish` module.
- **Permitted call-sites:** the public per-file-kind functions
  (`publish_feature_diagnostics`, `publish_rust_diagnostics`, and any future
  variant). New diagnostic publishers must delegate to `publish_with` with a
  compute closure rather than re-implementing the guards or notify call.
- **Composition rules:** the compute closure returns
  `Option<Vec<Diagnostic>>` — `None` skips publishing entirely (used when a
  feature file has no index, preserving previously published diagnostics), while
  `Some(vec![])` still publishes, so stale diagnostics are cleared.
  `prepare_publish` separates parameter construction from the notify side
  effect, so tests can pin payloads without a client socket.

The published payloads for representative feature and Rust files are pinned by
`insta` snapshots, and the publish invariants (count preserved, empty vector
still published) by a property test, both in `handlers/diagnostics/publish.rs`.
The boundary itself — that `publish_with` actually reaches (or, for a skip,
never reaches) `ClientSocket::notify` — is proven through a real client socket,
which `prepare_publish`-only tests cannot observe:

- **Missing-index skip** is a transport-backed unit test in
  `handlers/diagnostics/publish.rs`: a real `async_lsp` main loop supplies the
  `ClientSocket`, `publish_feature_diagnostics` is called for a path with no
  feature index, and the captured outgoing traffic contains a sentinel
  notification but no `publishDiagnostics` — so the absent index skips the
  whole boundary rather than clearing prior client diagnostics.
- **Emission and empty-vector clearing** for both feature and Rust files are
  exercised end-to-end against a live server by the `smoke_lsp` integration
  suite: an unimplemented/unused step yields a non-empty `publishDiagnostics`,
  and resolving it re-publishes an empty array for the same URI.

## cargo-bdd scenario output formatting

`crates/cargo-bdd/src/output.rs` owns the rendering of skipped-scenario and
bypassed-step listings.

- **Options:** construct `ScenarioDisplayOptions` via the named constructors —
  `compact()` (`cargo bdd skipped`), `with_reasons()`
  (`cargo bdd skipped --reasons`), or `step_listing_appendix()`
  (`cargo bdd steps`). Do not build the struct with positional booleans at
  call-sites; add a new constructor when a new mode is needed.
- **Canonical formatter:** `format_scenario_line` renders one scenario line.
  The location, tag, and reason fragments come from the shared
  `format_location`, `append_tags`, and `append_reason` helpers — also used by
  `write_bypassed_steps` — gated by the display options. There are no per-mode
  `*_scenario_*` duplicates; new fragments belong in the shared helpers, gated
  in `format_scenario_line`.
- Rendered output per mode is pinned by `insta` snapshots, and the structural
  invariants (empty tag list emits no `[tags: …]` fragment; `:line` suffix
  appears only when requested and known; the leading separator newline requires
  both that it was requested and that at least one skipped scenario is
  rendered) by property tests, both in the `output::tests` module at
  `crates/cargo-bdd/src/output/tests.rs`. `write_scenarios` filters out
  scenarios that are not `Skipped` and returns early when none remain, so
  filtering out every skipped scenario suppresses the separator along with the
  listing.

## Attribute-policy conformance check

The canonical conformance test for `AttributePolicy` implementations —
`assert_attribute_policy_conformance::<P>(expected_rendered)`, in the
`rstest_bdd_harness::policy_conformance` module — pins three invariants for any
policy `P`:

1. **Emit** — the policy emits exactly the expected number of attributes.
2. **Render** — each attribute renders to the corresponding expected string,
   in order.
3. **rstest is first** — the first attribute path is `rstest::rstest`, so
   fixture expansion precedes the runtime-specific test macro.

Harness adapter crates (`rstest-bdd-harness-tokio`, `rstest-bdd-harness-gpui`,
and any future adapter) must exercise their policy through this helper,
supplying only the crate-specific expected rendered attributes; do not
re-implement the emit/render/ordering assertions per crate. New harness crates
get the policy contract for free by calling the helper from one `#[test]`.

The GPUI adapter's library target sets `test = false`, so an in-module
`#[cfg(test)]` block there would never be compiled or run. Its conformance test
therefore lives in the `tests/attribute_policy_behaviour.rs` integration
target, exercised under `--all-features` (which enables the crate's
`native-gpui-tests` feature) — the arrangement `make test` uses.

## Canonical step-keyword table

`crates/rstest-bdd-patterns/src/keyword.rs` drives the string ↔ `StepKeyword`
correspondence from a single `keyword_table![...]` invocation — the list of
`(rendering, variant)` pairs and the true source of truth. That macro generates
the `KEYWORDS` const table consumed by `StepKeyword::from_str`
(case-insensitive, whitespace-trimming parsing) and the match inside
`StepKeyword::as_str` (rendering), so neither side carries its own literal list.

- **Adding or renaming a keyword:** edit the `keyword_table![...]` invocation
  (and the enum variant) only — not the generated `KEYWORDS` const. Do not add
  parallel literals to `as_str`, `from_str`, or call-sites.
- **Round-trip contract:** for every variant `kw`,
  `StepKeyword::from_str(kw.as_str()) == Ok(kw)`; parsing accepts any ASCII
  case permutation and surrounding whitespace. The contract is pinned by the
  property suite in `crates/rstest-bdd-patterns/tests/keyword_props.rs`.
- Entries store the canonical title-case rendering used in generated output
  and diagnostics.

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
  and `registry/async_lookup.rs`. New lookup variants must resolve a step (via
  `resolve_exact_step` / `resolve_step`) and pass it through
  `mark_and_project`; calling `mark_used` directly from a lookup is a bug.
- The invariant is pinned across all variants by the property suite in
  `crates/rstest-bdd/tests/registry_mark_used_props.rs`. A `kani` harness was
  considered and omitted: the registry is backed by link-time `inventory`
  registration and a lazily built hash map, which a bounded harness cannot
  model cheaply, and the property suite already exercises every variant against
  hit and miss lookups.

## Internal APIs and tooling (ADR-010 to ADR-013)

ADR-010 remains proposed build-tooling work. ADR-011 records historical
scenario-state work that did not ship. ADR-012 is accepted and implemented in
v0.7.0, while ADR-013 is accepted and governs the current Whitaker lint gate.
They are summarized here so the decisions are discoverable from the developer
guide; the ADRs remain the authoritative source, and the planning rationale
lives in
[`docs/execplans/adopt-v0-6-0-beta2-feedback.md`](execplans/adopt-v0-6-0-beta2-feedback.md).

### Historical scenario-state helper proposal (ADR-011)

[ADR-011](adr-011-first-party-scenario-state-and-cleanup.md) proposed a
first-party replacement for the hand-rolled thread-local `RefCell` plus `Drop`
cleanup guard used by stateful GPUI scenarios under v0.6.x. The proposal did
not ship and was superseded by ADR-012 before implementation:

- A generic `ScenarioStore<T>` core would have lived in `rstest-bdd`, exposing
  `set`/`with`/`with_mut`/`take`/`reset` and wrapping the two-sided reset
  protocol. It is named to avoid colliding with the already-shipped
  `pub trait ScenarioState` and `pub struct Slot<T>` in
  `crates/rstest-bdd/src/state.rs`; it composes with `Slot<T>` rather than
  shadowing it.
- A `GpuiScenarioStore` specialization plus a cleanup-guard fixture macro would
  have shipped from `rstest-bdd-harness-gpui`. The proposed layering was
  acyclic: the harness crate already depends on `rstest-bdd`, and the core
  would not import the harness.
- The proposal required reset before assignment and `Drop` cleanup on success,
  assertion failure, and skip, covered by unit, property-based (`proptest`), and
  `serial_test`-guarded thread-isolation tests — see the ADR's _Testing
  strategy_.

Roadmap items 10.3.1 and 10.3.2 retain this proposal as an explicitly
superseded historical record; design coverage is in `rstest-bdd-design.md`
§2.7.6.4.

### Guard-based `StepContext` borrowing and `FixtureBorrowError` (ADR-012)

[ADR-012](adr-012-guard-based-stepcontext-borrowing.md) records the accepted
and implemented v0.7.0 redesign. Contributors touching the borrow machinery
should preserve these contracts:

- `StepContext::try_borrow` and `try_borrow_mut` take `&self` and return opaque
  `FixtureRef` and `FixtureRefMut` guards. Guards for distinct mutable fixtures
  can coexist (for example `&mut TestAppContext` alongside `&mut World`), while
  conflicting guards for the same key fail.
- The `try_*` APIs return `Result` with `FixtureBorrowError::NotFound`,
  `TypeMismatch`, `AlreadyBorrowed`, or `NotMutable`. The `borrow_ref` and
  `borrow_mut` methods remain `Option`-returning conveniences over those APIs.
- Step-returned overrides take precedence for guard-based access. `get::<T>`
  serves shared fixture storage only and deliberately ignores overrides.
- `FixtureRef` and `FixtureRefMut` expose stable opaque accessor and trait
  surfaces. The framework creates a fresh context and fresh framework-owned
  fixture cells per scenario, then enforces their cleanup at the scenario
  boundary on success, failure (unwinding), and skip. The `rstest` fixture
  scopes, including intentional `#[once]` sharing, remain unchanged; the v0.6
  thread-local reset protocol is historical.
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
  behind a `nightly` feature gate once stabilized.
- Invalidation must be a _tested contract_: a portability-aware rebuild
  regression test, a `trybuild` compile-time test for the emitted binding, and
  redacted `insta` snapshots for any touched diagnostic — see the ADR's
  _Testing strategy_. This is distinct from the OUT_DIR AST _caching_
  performance idea in `rstest-bdd-design.md` §3.2.2.

Tracked by roadmap item 10.3.3 (pulled forward to v0.6.0 final); design
coverage is in `rstest-bdd-design.md` §2.7.6.6. Until it lands,
`v0-6-0-migration-guide.md` carries a caveat that `.feature`-only edits do not
trigger a rebuild.

### Whitaker Dylint suite lint gate (ADR-013)

[ADR-013](adr-013-adopt-whitaker-no-unwrap-or-else-panic.md) introduced the
first Whitaker lint; the repository now runs the full Whitaker Dylint suite as
part of `make lint`, matching the estate-wide rollout that began with
leynos/netsuke#410.

Local setup installs the `whitaker` wrapper and its pinned Dylint driver
toolchain via the installer:

```bash
cargo install --locked whitaker-installer --version 0.2.6
whitaker-installer
```

`make lint` then runs `make lint-whitaker` after Clippy, which invokes:

```bash
RUSTFLAGS="-D warnings" whitaker --all -- --workspace --all-targets --all-features
```

The repository's ordinary build, test, and Clippy commands remain on the stable
toolchain. The pinned nightly used by the Dylint driver is managed by
`whitaker-installer` and scoped to lint runs.

Per-lint configuration lives in the root `dylint.toml`. Every `excluded_crates`
entry carries a rationale comment; keep that discipline when adding entries.
In-source `allow`/`expect` attributes do not suppress `no_std_fs_operations`
findings, so exclusions are the only sanctioned escape hatch for
legitimately-ambient code such as test-support crates and integration test
crates.

#### Fixture-expansion lint allowance

`crates/rstest-bdd-test-macros` owns the narrow `unused_braces` allowance
needed for `rstest` fixture expansion. Apply its
`#[rstest_bdd_test_macros::allow_fixture_expansion_lints]` attribute
immediately above `#[fixture]` in workspace test code. It emits the allowance
only for that fixture and pairs it with Clippy's conditional `allow_attributes`
expectation.

The crate is a test-only development dependency: production crates and
non-fixture test helpers must not depend on it. Do not extend the attribute to
other lints or apply it to arbitrary functions; add a separately justified,
scoped mechanism if another macro expansion needs one.

When maintaining the pin:

1. Update `WHITAKER_INSTALLER_VERSION` in `.github/workflows/ci.yml`; the
   suite itself is rolling and updated by rerunning `whitaker-installer`.
2. Re-run `make lint-whitaker`, then the full `make lint` gate.
3. Update ADR-013 only if the mechanism or adopted lint set changes.

The root `clippy.toml` sets `allow-expect-in-tests = true` and
`allow-panic-in-tests = true`. These keys are narrowly scoped: recognized
built-in `#[test]` and `rstest` cases may use `.expect(...)` and `panic!(...)`
at their test boundary; they do not permit `.unwrap()` or use in shared helpers.

Outside recognized test cases, do not replace invariant checks with
`.expect(...)`, `.unwrap()`, or `unwrap_or_else(|| panic!(...))`. Use a
copyable invariant check such as
`let Some(value) = value else { panic!("expected value to be present"); };`, or
return `Result` and use `?` when an operation is fallible. Recognized built-in
`#[test]` and `rstest` cases may use `.expect(...)` or `panic!(...)` for
unexpected setup failures, aligned with ADR-013; `.unwrap()` and
`unwrap_or_else(|| panic!(...))` remain disallowed. Their signatures need not
return `Result` simply to propagate fixture errors. Reusable fixture functions
and shared helpers should still return `Result` rather than panic. Shared
assertion shapes belong in macros so panic line numbers point at the calling
test.

## Step-return overrides and `InsertOutcome` (ADR-015)

[ADR-015](adr-015-insert-outcome-for-step-return-overrides.md) records why
`StepContext::insert_value` returns `InsertOutcome` rather than
`Option<Box<dyn Any>>`. When a step function returns a value, the runner offers
it as an override for the fixture of the same type; `insert_value` reports what
became of it:

- `Inserted(previous)` — a fixture uniquely matched the value's type and the
  override was recorded. `previous` carries the displaced override, or `None`
  when it displaced nothing.
- `NoMatch` — no fixture has the value's type, so the value was dropped.
- `AmbiguousIgnored` — several fixtures match, so the value was dropped rather
  than bound to an arbitrary one.

The old `Option` return collapsed the last two into the same `None` as a
successful insert that displaced nothing, which is why a silently discarded
step return was invisible at the call site.

Two points matter when touching this code:

- `InsertOutcome` is `#[must_use]`. The generated scenario runner is the one
  place where dropping the outcome is correct, and it says so explicitly with
  `let _ = ctx.insert_value(…)` rather than relying on an implicit discard.
  Suppressing the warning anywhere else should be justified in review.
- Prefer the accessors over matching where the distinction does not matter.
  `into_previous()` consumes the outcome and yields the displaced override —
  the old `Option` result exactly — and `is_inserted()` answers the boolean
  question without consuming it.

The dropped cases remain dropped: the ADR changed what a caller can observe,
not the runtime policy for unmatched or ambiguous types. Caller-facing upgrade
steps are in the [v0.6.0 migration guide](v0-6-0-migration-guide.md).

## Language-server handler conventions

### Canonical extension predicate: `has_extension`

`rstest_bdd_server::handlers::util::has_extension(path, ext)` is the single
canonical predicate for testing a path's file extension in handler code. It
compares the path's final extension against `ext` (supplied without a leading
dot) using ASCII case-insensitive equality, and returns `false` for paths with
no extension.

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
behaviour for missing, repeated, and trailing dots) are pinned by the property
suite in `crates/rstest-bdd-server/tests/has_extension_props.rs`.
