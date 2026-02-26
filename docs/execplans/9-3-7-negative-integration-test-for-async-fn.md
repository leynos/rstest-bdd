# ExecPlan 9.3.7: negative trybuild test for `async fn` + `TokioHarness`

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

## Purpose / big picture

Roadmap item 9.3.7 requires a compile-fail integration test proving that
combining `harness = rstest_bdd_harness_tokio::TokioHarness` with an
`async fn` scenario signature produces a clear compile-time diagnostic. An
existing test (`scenario_harness_async_rejected.rs`) already verifies this
rejection for `StdHarness`, but there is no coverage for `TokioHarness` — the
primary harness users will reach for. Without this test, a future refactor
could silently break the rejection path for the Tokio-specific harness without
any test catching it.

After this change, a developer running `make test` will see the new trybuild
fixture `scenario_harness_tokio_async_rejected.rs` exercised inside the
`step_macros_compile` test. The fixture asserts the diagnostic message:

```plaintext
error: combining `harness` with `async fn` scenarios is not supported;
use a synchronous scenario function with `TokioHarness` instead
(the harness provides the Tokio runtime for step functions)
```

Success is observable when `make test` passes and the new `.stderr` snapshot
matches the compiler output exactly.

## Constraints

- Implement only roadmap item 9.3.7 from `docs/roadmap.md`.
- Do not modify any public API, trait, or type signature.
- Do not add `rstest-bdd-harness-tokio` as a dependency of the fixture crate
  (`crates/rstest-bdd/tests/fixtures_macros/Cargo.toml`). The `compile_error!`
  fires during macro expansion before type resolution, so the dependency is
  unnecessary and adding it would be misleading. The existing passing fixture
  `scenario_attributes_tokio.rs` already demonstrates that
  `rstest_bdd_harness_tokio` paths resolve without this dependency.
- Do not modify the compile-time check in
  `crates/rstest-bdd-macros/src/codegen/scenario.rs`. The check already covers
  all harness types (`config.harness.is_some() && config.runtime.is_async()`).
- No file may exceed 400 lines (AGENTS.md rule).
- All comments and documentation must use en-GB-oxendict spelling.
- Required quality gates: `make check-fmt`, `make lint`, `make test`.
- Capture gate output with `set -o pipefail` and `tee` to log files.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 6 files or 50 net
  lines of code, stop and escalate.
- Interface: if any public trait or type signature must change, stop and
  escalate.
- Dependencies: if a new external dependency is required, stop and escalate.
- Iterations: if the `.stderr` snapshot still does not match after 3 attempts
  at correction, stop and escalate with the diff.
- Ambiguity: if the compiler output for `TokioHarness` differs unexpectedly
  from the `StdHarness` variant (different span, different message), stop and
  investigate before proceeding.

## Risks

- Risk: the `.stderr` expected output may differ subtly between toolchain
  versions (line numbers, span rendering, nightly hint wording).
  Severity: low. Likelihood: low. Mitigation: generate the `.stderr` by
  running trybuild with `TRYBUILD=overwrite`, then review and adopt the
  generated output rather than handwriting it.

- Risk: trybuild may attempt to resolve the `rstest_bdd_harness_tokio` path
  even though `compile_error!` fires first, causing a dependency error.
  Severity: low. Likelihood: very low. Mitigation: the `compile_error!` is
  emitted as an early return in the proc macro before any code referencing the
  harness type is generated. The existing `scenario_attributes_tokio.rs` test
  confirms paths in macro attributes resolve in this environment. If this
  risk materializes, add `rstest-bdd-harness-tokio` to the fixture crate's
  `Cargo.toml` as a targeted fix.

## Progress

- [x] (2026-02-25) Stage A: verified assumptions (read-only).
- [x] (2026-02-25) Stage B: created fixture file and `.stderr` snapshot.
- [x] (2026-02-25) Stage C: registered fixture in `trybuild_macros.rs`.
- [x] (2026-02-25) Stage D: validated `.stderr` with `TRYBUILD=overwrite`
  — handwritten snapshot matched compiler output exactly.
- [x] (2026-02-25) Stage E: updated roadmap; design doc and users guide
  already cover `TokioHarness` async rejection.
- [x] (2026-02-25) Stage F: all quality gates passed.

## Surprises & discoveries

- Observation: the handwritten `.stderr` snapshot matched the compiler
  output on the first attempt. No iteration was needed.
  Evidence: `TRYBUILD=overwrite` run passed without generating a
  replacement file. Impact: none; confirmed the approach was sound.

## Decision log

- Decision: name the fixture `scenario_harness_tokio_async_rejected.rs`.
  Rationale: follows the existing pattern where the harness variant appears
  after `harness_` and before the condition. The existing file is
  `scenario_harness_async_rejected` (generic/StdHarness). Inserting `tokio_`
  distinguishes the harness type and reads naturally as "scenario harness
  (Tokio) async rejected". Date/Author: 2026-02-25 / DevBoxer.

- Decision: do not add `rstest-bdd-harness-tokio` to the fixture crate
  `Cargo.toml`. Rationale: the `compile_error!` fires during macro expansion
  before type resolution, so the type path never needs to resolve in the
  fixture crate. Adding it would imply a false requirement.
  Date/Author: 2026-02-25 / DevBoxer.

- Decision: the `.stderr` content will be nearly identical to the existing
  `scenario_harness_async_rejected.stderr`, differing only in the fixture file
  name and the harness path on line 15. Rationale: the error message is a
  literal string in `scenario.rs` (lines 176–179) and does not interpolate the
  harness type. The span points to `call_site()`, covering the
  `#[scenario(...)]` attribute. Date/Author: 2026-02-25 / DevBoxer.

## Outcomes & retrospective

Delivered in 9.3.7:

- Added compile-fail fixture
  `scenario_harness_tokio_async_rejected.rs` verifying that
  `harness = rstest_bdd_harness_tokio::TokioHarness` combined with
  `async fn` scenario signatures produces the expected `compile_error!`
  diagnostic.
- Added matching `.stderr` snapshot.
- Registered the fixture in `trybuild_macros.rs`
  `run_failing_macro_tests`.
- Marked roadmap item 9.3.7 complete in `docs/roadmap.md`.

Validation summary:

- `make check-fmt` passed (`/tmp/9-3-7-check-fmt.log`).
- `make lint` passed (`/tmp/9-3-7-lint.log`).
- `make test` passed (`/tmp/9-3-7-test.log`), including the trybuild
  `step_macros_compile` test exercising all fixtures.
- `make markdownlint` passed (`/tmp/9-3-7-markdownlint.log`).
- `make nixie` passed (`/tmp/9-3-7-nixie.log`).

Result against finish line: trybuild test asserts the diagnostic message
for `TokioHarness + async fn`; `make test` passes. No risks materialized.
No tolerances were approached.

Retrospective: this was a straightforward task with no surprises. The
existing `StdHarness` negative test provided a clean template. The
handwritten `.stderr` matched on the first attempt because the error
message is a literal string and the fixture structure is identical.

## Context and orientation

The rstest-bdd project is a BDD (behaviour-driven development) testing
framework for Rust, organized as a Cargo workspace. The crates relevant to
this task are:

- **`crates/rstest-bdd-macros`** — the procedural macro crate. Contains the
  `#[scenario]` attribute macro. The compile-time check that rejects
  `harness + async fn` lives in
  `crates/rstest-bdd-macros/src/codegen/scenario.rs` at lines 174–182
  (regular scenarios) and 243–251 (outlines). The check is
  `if config.harness.is_some() && config.runtime.is_async()` — it fires for
  any harness type, not just `StdHarness`.

- **`crates/rstest-bdd`** — the runtime library. Its `tests/` directory
  contains the trybuild integration tests:
  - `tests/trybuild_macros.rs` — the test driver that registers fixture cases
    and runs them via the `step_macros_compile` test function.
  - `tests/fixtures_macros/` — directory containing `.rs` fixture files and
    their corresponding `.stderr` expected-output snapshots.
  - `tests/fixtures_macros/Cargo.toml` — the fixture crate manifest (depends
    only on `rstest-bdd` and `rstest-bdd-macros`; no harness crate
    dependency).
  - `tests/fixtures_macros/basic.feature` — the Gherkin feature file
    referenced by all fixtures via `include_str!`.

- **`crates/rstest-bdd-harness-tokio`** — the Tokio harness plugin crate.
  Exports `TokioHarness` (implements `HarnessAdapter`) and
  `TokioAttributePolicy` (implements `AttributePolicy`). The full path used
  in fixture code is `rstest_bdd_harness_tokio::TokioHarness`.

The existing negative test that serves as the template for this work:

- Fixture:
  `crates/rstest-bdd/tests/fixtures_macros/scenario_harness_async_rejected.rs`
- Expected stderr:
  `crates/rstest-bdd/tests/fixtures_macros/scenario_harness_async_rejected.stderr`
- Registration: `trybuild_macros.rs` line 98, inside `run_failing_macro_tests`

That fixture uses `harness = rstest_bdd_harness::StdHarness` with
`async fn async_with_harness()`. The new fixture mirrors it but substitutes
`rstest_bdd_harness_tokio::TokioHarness`.

## Plan of work

### Stage A: verify assumptions (read-only)

Goal: confirm the three key assumptions before writing any code.

1. The `compile_error!` check in `scenario.rs` does not reference the harness
   type — it checks `config.harness.is_some()` only. Confirmed by reading
   lines 174–182 of
   `crates/rstest-bdd-macros/src/codegen/scenario.rs`.

2. The fixture crate does not need `rstest-bdd-harness-tokio`. Confirmed: the
   error fires before type resolution, and the existing passing fixture
   `scenario_attributes_tokio.rs` already uses
   `rstest_bdd_harness_tokio::TokioAttributePolicy` without this dependency
   in the fixture `Cargo.toml`.

3. The error message text will be identical for both harness types. Confirmed:
   it is a literal string, not interpolated with the harness path.

Go/no-go: all three assumptions hold. Proceed.

### Stage B: create fixture file and `.stderr` snapshot

Goal: produce the two new files that define the compile-fail test.

Create `crates/rstest-bdd/tests/fixtures_macros/scenario_harness_tokio_async_rejected.rs`:

```rust
//! Compile-fail fixture: `TokioHarness` combined with `async fn` is rejected.
use rstest_bdd_macros::{given, scenario, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(
    path = "basic.feature",
    harness = rstest_bdd_harness_tokio::TokioHarness,
)]
async fn async_with_tokio_harness() {}

const _: &str = include_str!("basic.feature");

fn main() {}
```

Create `crates/rstest-bdd/tests/fixtures_macros/scenario_harness_tokio_async_rejected.stderr`.
The content mirrors the existing
`scenario_harness_async_rejected.stderr` with two differences:

1. The `-->` path references the new fixture file name.
2. Line 15 shows `harness = rstest_bdd_harness_tokio::TokioHarness,`
   instead of `harness = rstest_bdd_harness::StdHarness,`.

The error message, span shape, and nightly hint are identical.

This is derived from the existing stderr by changing:

1. The file name in the `-->` path (from `scenario_harness_async_rejected.rs`
   to `scenario_harness_tokio_async_rejected.rs`).
2. The content of line 15 (from `harness = rstest_bdd_harness::StdHarness,`
   to `harness = rstest_bdd_harness_tokio::TokioHarness,`).

The line numbers (13–16) are identical because the fixture has the same
structure and the `#[scenario(...)]` attribute starts on the same line.

Go/no-go: both files exist and have the expected content. Proceed.

### Stage C: register fixture in `trybuild_macros.rs`

Goal: wire the new fixture into the trybuild test driver.

Edit `crates/rstest-bdd/tests/trybuild_macros.rs`, function
`run_failing_macro_tests`. Add one line immediately after the existing
`scenario_harness_async_rejected.rs` entry (line 98), so that related tests
are grouped:

```rust
        MacroFixtureCase::from("scenario_harness_async_rejected.rs"),
        MacroFixtureCase::from("scenario_harness_tokio_async_rejected.rs"),
```

The file is currently 248 lines; adding one line brings it to 249, well under
the 400-line limit.

Go/no-go: the fixture is registered. Proceed.

### Stage D: validate `.stderr` with `TRYBUILD=overwrite`

Goal: confirm the handwritten `.stderr` matches the actual compiler output.

Run the trybuild test in overwrite mode to generate the actual stderr, then
compare:

```bash
set -o pipefail
TRYBUILD=overwrite cargo test --package rstest-bdd step_macros_compile \
  -- --exact 2>&1 | tee /tmp/9-3-7-trybuild-overwrite.log
```

If the generated file at
`target/tests/wip/scenario_harness_tokio_async_rejected.stderr` differs from
the handwritten version, adopt the generated version and investigate the
discrepancy.

If the `.stderr` needs correction, update it and re-run:

```bash
set -o pipefail
cargo test --package rstest-bdd step_macros_compile -- --exact 2>&1 \
  | tee /tmp/9-3-7-trybuild-verify.log
```

Go/no-go: the trybuild test passes with the new fixture. Proceed.

### Stage E: update documentation

Goal: keep roadmap, design doc, and users guide aligned with delivered
coverage.

**Roadmap** (`docs/roadmap.md`, line 521): change `- [ ]` to `- [x]`:

```markdown
- [x] 9.3.7. Add a negative integration test for `async fn` step definitions
```

**Design doc** (`docs/rstest-bdd-design.md`): sections 2.7.3 and 2.7.4
already describe the async rejection behaviour comprehensively. The "Async
rejection" paragraph in section 2.7.3 speaks generically about "combining
`harness` with `async fn`" and does not enumerate specific test fixtures. No
content change is required.

**Users guide** (`docs/users-guide.md`): the "Using the Tokio harness"
section already contains a note explaining that combining `harness` with
`async fn` scenario signatures produces a compile error, and specifically
mentions `TokioHarness`. No content change is required.

Go/no-go: documentation is accurate and complete. Proceed.

### Stage F: quality gates and commit

Goal: prove release-quality completion.

Run each gate with log capture:

```bash
set -o pipefail; make check-fmt 2>&1 | tee /tmp/9-3-7-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/9-3-7-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/9-3-7-test.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/9-3-7-markdownlint.log
```

All four commands must exit with status 0.

## Concrete steps

All commands run from the workspace root `/home/user/project`.

1. Create file
   `crates/rstest-bdd/tests/fixtures_macros/scenario_harness_tokio_async_rejected.rs`
   (content in Stage B).

2. Create file
   `crates/rstest-bdd/tests/fixtures_macros/scenario_harness_tokio_async_rejected.stderr`
   (content in Stage B).

3. Edit `crates/rstest-bdd/tests/trybuild_macros.rs`: add one line in
   `run_failing_macro_tests` after line 98 (content in Stage C).

4. Validate `.stderr` with `TRYBUILD=overwrite` and correct if needed
   (commands in Stage D).

5. Edit `docs/roadmap.md` line 521: change `- [ ]` to `- [x]`.

6. Run quality gates (commands in Stage F). Expected: all pass with exit
   code 0.

## Validation and acceptance

Primary acceptance criterion: `make test` passes, and the trybuild
`step_macros_compile` test exercises the new compile-fail fixture
`scenario_harness_tokio_async_rejected.rs`. The fixture must produce the
exact error message documented in the design doc §2.7.3.

Quality criteria:

- Tests: `make test` passes (`cargo test --workspace`). The trybuild test
  `step_macros_compile` exercises all registered fixtures including the new
  one.
- Lint: `make lint` passes
  (`cargo clippy --workspace --all-targets --all-features -- -D warnings`).
- Format: `make check-fmt` passes (`cargo fmt --workspace -- --check`).
- Markdown: `make markdownlint` passes.
- File lengths: no file exceeds 400 lines.

Quality method:

```bash
set -o pipefail && make check-fmt 2>&1 | tee /tmp/9-3-7-check-fmt.log \
  && make lint 2>&1 | tee /tmp/9-3-7-lint.log \
  && make test 2>&1 | tee /tmp/9-3-7-test.log \
  && make markdownlint 2>&1 | tee /tmp/9-3-7-markdownlint.log
```

Expected: all four commands exit with status 0.

## Idempotence and recovery

All steps are idempotent. Creating the fixture and `.stderr` files can be
repeated (overwriting previous content). The trybuild registration line can be
re-added if accidentally removed. If the `.stderr` file is wrong, use
`TRYBUILD=overwrite` to regenerate and adopt the correct output. No
destructive operations are involved.

## Artifacts and notes

Files created (2 new):

- `crates/rstest-bdd/tests/fixtures_macros/scenario_harness_tokio_async_rejected.rs`
- `crates/rstest-bdd/tests/fixtures_macros/scenario_harness_tokio_async_rejected.stderr`

Files modified (2 existing):

- `crates/rstest-bdd/tests/trybuild_macros.rs` (1 line added)
- `docs/roadmap.md` (1 character changed: `[ ]` → `[x]`)

Total net change: approximately 34 lines added, 0 removed. Well within
tolerances.

## Interfaces and dependencies

No new interfaces, traits, or dependencies. The fixture uses only existing
macro attributes (`#[given]`, `#[when]`, `#[then]`, `#[scenario]`) and
references the existing `rstest_bdd_harness_tokio::TokioHarness` path. The
`compile_error!` mechanism is already implemented in
`crates/rstest-bdd-macros/src/codegen/scenario.rs` and requires no
modification.
