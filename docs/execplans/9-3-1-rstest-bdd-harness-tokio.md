# ExecPlan 9.3.1: Create `rstest-bdd-harness-tokio`

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

## Purpose / big picture

Phase 9 of the roadmap implements Architecture Decision Record (ADR) 005 by
introducing a harness adapter layer so framework-specific integrations (Tokio,
Graphical Processing User Interface (GPUI), Bevy) live in opt-in crates rather
than the core runtime or macros. Phases 9.1 and 9.2 delivered the core harness
traits (`HarnessAdapter`, `AttributePolicy`) and macro integration
(`harness =`, `attributes =` parameters). Phase 9.3 delivers the first official
framework adapter: `rstest-bdd-harness-tokio`.

After this change, users can write:

    use rstest_bdd_macros::scenario;

    #[scenario(
        path = "tests/features/my_async.feature",
        harness = rstest_bdd_harness_tokio::TokioHarness,
        attributes = rstest_bdd_harness_tokio::TokioAttributePolicy,
    )]
    fn my_async_scenario() {
        // Steps execute inside a Tokio current-thread runtime.
    }

Success is observable when:

- A new crate `rstest-bdd-harness-tokio` exists, is a workspace member, and
  exports `TokioHarness` and `TokioAttributePolicy`.
- `TokioHarness` implements `HarnessAdapter` by building a Tokio
  current-thread runtime and executing the scenario runner inside it.
- `TokioAttributePolicy` implements `AttributePolicy` and emits
  `#[rstest::rstest]` and `#[tokio::test(flavor = "current_thread")]`.
- Unit tests, behavioural tests, and integration tests (using `#[scenario]`
  with the Tokio harness) all pass.
- `make check-fmt`, `make lint`, and `make test` succeed.
- `docs/rstest-bdd-design.md`, `docs/users-guide.md`, and `docs/roadmap.md`
  are updated.
- Roadmap items 9.3.1, 9.3.2, and 9.3.3 are marked done.

## Constraints

- Implement roadmap items 9.3.1, 9.3.2, and 9.3.3 only. Do not implement
  phase 9.4 (GPUI) or alter phase 9.2 behaviour.
- Keep Tokio out of core crates (`rstest-bdd`, `rstest-bdd-macros`,
  `rstest-bdd-harness`) per ADR-005. The new crate is the only place Tokio
  appears as a direct dependency.
- Preserve existing public behaviour. Existing
  `runtime = "tokio-current-thread"` compatibility paths and `#[scenario]`
  without harness must continue to work unchanged.
- Every new Rust module must begin with a `//!` module-level doc comment. All
  public APIs must have Rustdoc `///` comments with usage examples.
- Files must not exceed 400 lines.
- Use en-GB-oxendict spelling in documentation ("-ize" / "-yse" / "-our").
- Quality gates must pass before completion: `make check-fmt`, `make lint`,
  `make test`.
- Record design decisions in `docs/rstest-bdd-design.md`.
- Record user-facing usage in `docs/users-guide.md`.
- Mark roadmap entries done only after all gates pass.

## Tolerances (exception triggers)

- Scope: if delivery requires changing more than 20 files or more than 800 net
  lines of code, stop and escalate.
- Interfaces: if any existing public API in `rstest-bdd`, `rstest-bdd-macros`,
  or `rstest-bdd-harness` must be removed or made incompatible, stop and
  escalate.
- Dependencies: if a new external dependency is needed in core crates (not the
  new harness-tokio crate), stop and escalate.
- Behaviour: if existing async scenario behaviour or harness delegation
  behaviour regresses in tests, stop and escalate.
- Iterations: if the same failing gate (`check-fmt`, `lint`, or `test`) fails
  three times after attempted fixes, stop and escalate with logs.
- Ambiguity: if ADR-005, the design doc, and the roadmap conflict on interface
  shape, stop and request direction.

## Risks

- Risk: Tokio runtime nesting. If any test runner already has a Tokio runtime
  active, building a second runtime inside `TokioHarness::run()` would panic.
  Severity: medium. Likelihood: low (the harness is designed for sync scenario
  functions; the async+harness rejection is still in place). Mitigation: the
  `TokioHarness` creates its own runtime from a sync context; document this
  constraint.

- Risk: workspace Tokio feature mismatch. The workspace Tokio dependency uses
  `rt-multi-thread`, `macros`, `io-std`, `sync`. The harness-tokio crate needs
  only `rt`. Using workspace = true would pull unnecessary features. Severity:
  low. Likelihood: certain. Mitigation: declare a local Tokio dependency with
  only `features = ["rt"]` rather than `workspace = true`.

- Risk: strict Clippy lints. The workspace denies `expect_used`,
  `unwrap_used`, `indexing_slicing`, `string_slice`, and others. Severity: low.
  Likelihood: medium. Mitigation: use `unwrap_or_else(|err| panic!(...))` for
  the runtime build step, which is the established workspace pattern.

- Risk: documentation drift across design doc, user guide, roadmap, and
  releasing guide. Severity: medium. Likelihood: medium. Mitigation: update all
  four in the same stage.

## Progress

- [x] (2026-02-21 00:00Z) Drafted this ExecPlan.
- [x] (2026-02-21) Stage A: baseline validation (1171 Rust tests, 47 Python
  tests passed).
- [x] (2026-02-21) Stage B: scaffold crate and workspace wiring.
- [x] (2026-02-21) Stage C: implement `TokioHarness` with unit tests (9.3.2).
- [x] (2026-02-21) Stage D: implement `TokioAttributePolicy` with unit tests
  (9.3.3).
- [x] (2026-02-21) Stage E: add behavioural tests for the new crate.
- [x] (2026-02-21) Stage F: add integration test in `rstest-bdd` using
  `#[scenario]`.
- [x] (2026-02-21) Stage G: update documentation, roadmap, release guide, and
  publish script.
- [x] (2026-02-21) Stage H: final quality gates (1183 Rust tests, 47 Python
  tests passed; `make check-fmt`, `make lint`, `make test` all exit 0).

## Surprises & discoveries

- Observation: the workspace-level lint `missing_crate_level_docs` has been
  renamed to `rustdoc::missing_crate_level_docs` in recent Rust editions.
  Evidence: doctest runner emitted a `renamed_and_removed_lints` warning.
  Impact: cosmetic only; the warning does not block any quality gate and is
  emitted for all workspace crates, not specific to this change.

- Observation: `cargo fmt` applied different line-wrapping rules than the
  manually written code in test files (e.g., import grouping and method chain
  formatting). Evidence: `make check-fmt` failed on first attempt. Impact:
  resolved by running `make fmt` before the final gate; no functional change.

- Observation (code review): the `TokioHarness` does not enable async step
  *definitions* (i.e., `async fn` step functions). The existing
  `generate_sync_wrapper_from_async` in `emit.rs` checks
  `tokio::runtime::Handle::try_current().is_ok()` and rejects async steps when
  a Tokio runtime is already active. This means async step functions will fail
  at runtime inside `TokioHarness` because the harness establishes a runtime
  before steps execute. Evidence: `emit.rs:78-86`. Impact: the compile-time
  error message was updated to remove the "planned for phase 9.3" wording and
  accurately describe the current state. The user guide documents this
  limitation clearly.

## Decision log

- Decision: declare Tokio as a local dependency with `features = ["rt"]`
  rather than using `tokio.workspace = true`. Rationale: the workspace Tokio
  dependency pulls `rt-multi-thread`, `macros`, `io-std`, and `sync`, none of
  which are needed by the harness adapter. A minimal feature set keeps the
  crate lightweight per ADR-005 goals. Date/Author: 2026-02-21 / plan.

- Decision: use `LocalSet::block_on` with `yield_now()` after
  `request.run()`. Rationale: a plain `runtime.block_on` does not provide a
  `LocalSet` context, which means `tokio::task::spawn_local` would panic inside
  step functions. Wrapping execution in a `LocalSet` makes `spawn_local`
  available, and the `yield_now()` after `request.run()` gives the `LocalSet` a
  chance to drive any tasks spawned during step execution. Date/Author:
  2026-02-22 / code review (supersedes original `block_on`-only decision from
  2026-02-21).

- Decision: use `Builder::new_current_thread().enable_all().build()` for the
  runtime. Rationale: `enable_all()` activates time and I/O drivers, matching
  `#[tokio::test]` defaults. `new_current_thread()` matches the
  `flavor = "current_thread"` attribute emitted by `TokioAttributePolicy`.
  Date/Author: 2026-02-21 / plan.

- Decision: update compile-time error message for async+harness rejection to
  remove "planned for phase 9.3" and guide users towards using synchronous
  scenario functions with `TokioHarness`. Rationale: phase 9.3 is delivered;
  `TokioHarness` provides the Tokio runtime for step functions without
  requiring `async fn` scenario signatures. The `.stderr` fixture was updated
  to match. Date/Author: 2026-02-22 / code review.

## Outcomes & retrospective

Shipped in this phase:

- New crate `crates/rstest-bdd-harness-tokio` with `TokioHarness` and
  `TokioAttributePolicy`.
- `TokioHarness` builds a Tokio current-thread runtime with a `LocalSet` and
  executes the scenario runner inside it, making
  `tokio::runtime::Handle::current()` and `tokio::task::spawn_local` available
  in step functions.
- `TokioAttributePolicy` emits `#[rstest::rstest]` and
  `#[tokio::test(flavor = "current_thread")]`.
- 5 unit tests (2 harness, 3 policy) with doctests.
- 7 behavioural tests (5 harness execution including async task completion, 2
  policy output).
- 2 integration tests in `rstest-bdd`: one proving `#[scenario]` with
  `TokioHarness` works end-to-end (including `spawn_local` current-thread
  proof), one combining `TokioHarness` with `TokioAttributePolicy`.
- Documentation updates in `docs/rstest-bdd-design.md`,
  `docs/users-guide.md`, `docs/roadmap.md`, `docs/releasing-crates.md`, and
  `scripts/publish_workspace_members.py`.

Test count grew from 1171 to 1183 (12 new tests).

Validation summary:

- `make check-fmt` passed.
- `make lint` passed.
- `make test` passed (1183 Rust tests, 47 Python tests).

Deferred to future phases:

- Phase 9.4: GPUI harness plugin crate.
- Async scenario function support with harness: the compile-time rejection of
  `async fn` + `harness` remains in place. `TokioHarness` provides a Tokio
  runtime for synchronous step functions; async step *definitions* are not
  supported because `generate_sync_wrapper_from_async` in `emit.rs` rejects
  when a Tokio runtime is already active. Lifting this would require an async
  variant of `HarnessAdapter` or changes to the sync wrapper logic.

## Context and orientation

The `rstest-bdd` workspace (`/home/user/project`) is a Rust Behaviour-Driven
Development (BDD) testing framework built on `rstest`. It lives in a Cargo
workspace with edition 2024, version 0.5.0, and minimum supported Rust version
(MSRV) 1.85. The workspace root `Cargo.toml` is at
`/home/user/project/Cargo.toml`.

Key crates for this work:

- `crates/rstest-bdd-harness/` — defines the core traits and types:
  - `src/adapter.rs`: `HarnessAdapter` trait with
    `fn run<T>(&self, request: ScenarioRunRequest<'_, T>) -> T`.
  - `src/policy.rs`: `AttributePolicy` trait, `TestAttribute` struct,
    `DefaultAttributePolicy`.
  - `src/runner.rs`: `ScenarioMetadata`, `ScenarioRunner<'a, T>`,
    `ScenarioRunRequest<'a, T>`.
  - `src/std_harness.rs`: `StdHarness` — the default sync harness.
  - `tests/harness_behaviour.rs`: behavioural tests for harness execution.
  - `tests/attribute_policy_behaviour.rs`: behavioural tests for policy output.

- `crates/rstest-bdd/` — the main framework runtime. Its `Cargo.toml` already
  has `rstest-bdd-harness.workspace = true` in dev-dependencies.
  - `tests/scenario_harness.rs`: integration tests for `#[scenario]` with
    `harness =` and `attributes =` parameters using `StdHarness`.
  - `tests/async_scenario.rs`: integration tests for
    `runtime = "tokio-current-thread"`.
  - `tests/runtime_compat_alias.rs`: behavioural tests for the compatibility
    alias.

- `crates/rstest-bdd-macros/` — the proc-macro crate. Already supports
  `harness = path::ToHarness` and `attributes = path::ToPolicy` in
  `#[scenario]` and `scenarios!`.

Documentation:

- `docs/roadmap.md` lines 473–477: items 9.3.1, 9.3.2, 9.3.3.
- `docs/rstest-bdd-design.md` section 2.7.4 (lines 1618–1631): describes
  first-party plugin targets including `rstest-bdd-harness-tokio`.
- `docs/users-guide.md` lines 732–825: harness adapter section with the
  "Async limitation (pending phase 9.3)" blockquote.
- `docs/releasing-crates.md`: publication order (currently omits harness
  crates).
- `scripts/publish_workspace_members.py`: `PUBLISHABLE_CRATES` tuple
  (currently omits harness crates).

Build system:

- `make check-fmt` — verify formatting.
- `make lint` — Clippy with strict denials.
- `make test` — full workspace test suite.
- `make markdownlint` — Markdown validation.

## Plan of work

### Stage A: baseline validation

Run the full test suite to confirm the workspace is green before edits. Capture
the output to a log file for comparison.

Go/no-go: `make test` exits 0.

### Stage B: scaffold crate and workspace wiring

Create the directory `crates/rstest-bdd-harness-tokio/` with:

- `Cargo.toml` — workspace-inherited metadata, dependency on
  `rstest-bdd-harness` (workspace) and `tokio` (version "1", features =
  ["rt"]), dev-dependency on `rstest` (workspace).
- `README.md` — brief description of the crate.
- `src/lib.rs` — module-level doc comment, module declarations, and public
  re-exports (initially empty, filled in stages C and D).

Update the workspace root `Cargo.toml`:

- Add `"crates/rstest-bdd-harness-tokio"` to `[workspace] members`.
- Add workspace dependency and patch entry.

Go/no-go: `cargo check --workspace` exits 0.

### Stage C: implement `TokioHarness` (roadmap 9.3.2)

Create `crates/rstest-bdd-harness-tokio/src/tokio_harness.rs` with the
`TokioHarness` struct and `HarnessAdapter` implementation. Add inline unit
tests.

Go/no-go: `cargo test -p rstest-bdd-harness-tokio` passes.

### Stage D: implement `TokioAttributePolicy` (roadmap 9.3.3)

Create `crates/rstest-bdd-harness-tokio/src/policy.rs` with
`TokioAttributePolicy` and `AttributePolicy` implementation. Add inline unit
tests.

Go/no-go: `cargo test -p rstest-bdd-harness-tokio` passes.

### Stage E: add behavioural tests for the new crate

Create `tests/harness_behaviour.rs` and `tests/attribute_policy_behaviour.rs`
mirroring the base harness crate's behavioural test structure.

Go/no-go: `cargo test -p rstest-bdd-harness-tokio` passes with all new tests.

### Stage F: add integration test in `rstest-bdd`

Add `rstest-bdd-harness-tokio` as a dev-dependency of `rstest-bdd`. Create a
feature file and integration test proving `#[scenario]` with
`harness = rstest_bdd_harness_tokio::TokioHarness` works end-to-end.

Go/no-go: `cargo test -p rstest-bdd --test scenario_harness_tokio` passes.

### Stage G: update documentation

Update design doc, user guide, roadmap, release guide, and publish script.

Go/no-go: documentation reads correctly.

### Stage H: final quality gates

Run `make check-fmt`, `make lint`, and `make test`.

Go/no-go: all three commands exit 0.

## Concrete steps

All commands run from the repository root `/home/user/project`.

1. Baseline validation.

       set -o pipefail && make test 2>&1 | tee /tmp/9-3-1-baseline-test.log

2. Create crate directory and files, update workspace wiring.

       cargo check --workspace 2>&1 | tee /tmp/9-3-1-scaffold-check.log

3. Implement source modules and unit tests.

       cargo test -p rstest-bdd-harness-tokio \
         2>&1 | tee /tmp/9-3-1-crate-tests.log

4. Add behavioural tests.

       cargo test -p rstest-bdd-harness-tokio \
         2>&1 | tee /tmp/9-3-1-behaviour-tests.log

5. Add integration test in `rstest-bdd`.

       cargo test -p rstest-bdd --test scenario_harness_tokio \
         2>&1 | tee /tmp/9-3-1-integration-test.log

6. Update documentation files and scripts.

7. Final quality gates.

       set -o pipefail && make check-fmt 2>&1 | tee /tmp/9-3-1-check-fmt.log
       set -o pipefail && make lint 2>&1 | tee /tmp/9-3-1-lint.log
       set -o pipefail && make test 2>&1 | tee /tmp/9-3-1-test.log

## Validation and acceptance

Acceptance criteria:

- `crates/rstest-bdd-harness-tokio` exists and is a workspace member.
- The crate exports `TokioHarness` and `TokioAttributePolicy`.
- `TokioHarness` implements `HarnessAdapter` by running the scenario inside a
  Tokio current-thread runtime (verified by a test calling
  `tokio::runtime::Handle::current()` inside the harness).
- `TokioAttributePolicy` emits `#[rstest::rstest]` and
  `#[tokio::test(flavor = "current_thread")]` in that order.
- Unit tests in the new crate validate type contracts.
- Behavioural tests validate execution semantics and policy output from a
  consumer perspective.
- An integration test in `rstest-bdd` proves `#[scenario]` with
  `harness = rstest_bdd_harness_tokio::TokioHarness` works end-to-end.
- `docs/rstest-bdd-design.md` records the implemented plugin.
- `docs/users-guide.md` explains usage and removes the phase 9.3 limitation
  note.
- `docs/roadmap.md` marks 9.3.1, 9.3.2, and 9.3.3 as done.
- `docs/releasing-crates.md` and `scripts/publish_workspace_members.py`
  include the new crate.
- `make check-fmt`, `make lint`, and `make test` all succeed.

## Idempotence and recovery

Most steps are repeatable. If a gate fails:

- Inspect the corresponding `/tmp/9-3-1-*.log`.
- Fix the smallest local cause.
- Re-run only the failed command.
- Re-run the full required gates at the end.

If the crate scaffold is partially created, delete
`crates/rstest-bdd-harness-tokio/` and revert workspace `Cargo.toml` changes to
restart cleanly.

## Artifacts and notes

Expected evidence files:

- `/tmp/9-3-1-baseline-test.log`
- `/tmp/9-3-1-scaffold-check.log`
- `/tmp/9-3-1-crate-tests.log`
- `/tmp/9-3-1-behaviour-tests.log`
- `/tmp/9-3-1-integration-test.log`
- `/tmp/9-3-1-check-fmt.log`
- `/tmp/9-3-1-lint.log`
- `/tmp/9-3-1-test.log`

## Interfaces and dependencies

Target interface surface for `rstest-bdd-harness-tokio`:

In `crates/rstest-bdd-harness-tokio/src/tokio_harness.rs`:

    #[derive(Debug, Clone, Copy, Default)]
    pub struct TokioHarness;

    impl TokioHarness {
        #[must_use]
        pub const fn new() -> Self { Self }
    }

    impl HarnessAdapter for TokioHarness {
        fn run<T>(&self, request: ScenarioRunRequest<'_, T>) -> T {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap_or_else(|err| panic!(
                    "rstest-bdd-harness-tokio: failed to build Tokio \
                     runtime: {err}"
                ));
            runtime.block_on(async { request.run() })
        }
    }

In `crates/rstest-bdd-harness-tokio/src/policy.rs`:

    pub struct TokioAttributePolicy;

    const TOKIO_TEST_ATTRIBUTES: [TestAttribute; 2] = [
        TestAttribute::new("rstest::rstest"),
        TestAttribute::with_arguments(
            "tokio::test",
            "flavor = \"current_thread\"",
        ),
    ];

    impl AttributePolicy for TokioAttributePolicy {
        fn test_attributes() -> &'static [TestAttribute] {
            &TOKIO_TEST_ATTRIBUTES
        }
    }

Dependency constraints:

- `rstest-bdd-harness-tokio` depends on `rstest-bdd-harness` (workspace) and
  `tokio` (version "1", features = ["rt"]).
- No new dependencies are added to core crates.
- `rstest-bdd` gains `rstest-bdd-harness-tokio` as a dev-dependency only.

## Revision note

Initial draft created from roadmap phase 9.3, ADR-005, design document section
2.7.4, and prior ExecPlans 9-1-1 and 9-2-3. All interface names and file paths
were verified against the current working tree.
