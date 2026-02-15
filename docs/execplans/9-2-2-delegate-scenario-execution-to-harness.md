# Execution Plan (ExecPlan) 9.2.2: Delegate scenario execution to the selected harness adapter

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: IN PROGRESS

`PLANS.md` is not present in the repository at the time of writing, so this
ExecPlan is the governing plan for this task.

## Purpose / big picture

Phase 9.2.1 (complete) extended the `#[scenario]` and `scenarios!` macros to
accept `harness = path::ToHarness` and `attributes = path::ToPolicy`
parameters. Those parameters produce compile-time trait-bound assertions that
validate the user's types implement `HarnessAdapter` and `AttributePolicy`,
but the generated test body still executes steps and the scenario block
directly, without involving the harness adapter at runtime.

After this work, when `harness = SomeHarness` is specified, the generated test
function will:

1. Construct a `ScenarioMetadata` value from the scenario's compile-time
   constants (feature path, scenario name, line number, tags).
2. Wrap the runtime portion of the test (context setup, step executor loop,
   skip handler, context postlude, and user block) inside a
   `ScenarioRunner<'_, T>` closure.
3. Bundle the metadata and runner into a `ScenarioRunRequest`.
4. Instantiate the harness via `<HarnessType as Default>::default()`.
5. Call `harness.run(request)`, delegating execution to the adapter.

This completes the "delegation" pattern from ADR-005 and enables third-party
harness adapters (Tokio, GPUI, Bevy) to intercept scenario execution, inject
framework-specific fixtures, set up runtimes, and perform cleanup around the
scenario closure.

Success is observable when:

- A custom `RecordingHarness` integration test proves the harness `run()`
  method is called exactly once and receives correct `ScenarioMetadata`.
- `StdHarness` delegation produces identical test outcomes to the non-harness
  path (the existing `scenario_with_harness` test continues to pass).
- Fallible scenarios (`Result<(), E>` return type) compose correctly through
  the harness closure.
- Combining `harness` with an `async fn` scenario produces a clear
  `compile_error!` message directing users to phase 9.3.
- A harness type that does not implement `Default` produces a clear compile
  error.
- All existing tests pass unchanged when no harness is specified.
- `make check-fmt`, `make lint`, and `make test` all exit with status 0.
- Roadmap entry 9.2.2 is marked done.

## Constraints

- Implement only the 9.2.2 scope from `docs/roadmap.md`: delegate scenario
  execution to the harness adapter when `harness` is specified. Do not
  implement 9.2.3 (runtime compatibility alias) or 9.3 (Tokio harness plugin
  crate) in this change.
- Preserve exact backward compatibility: when `harness` is omitted, the
  generated code must be identical to the current output. Delegation is an
  additive code path, not a replacement.
- Keep Tokio and GPUI dependencies out of core crates (`rstest-bdd`,
  `rstest-bdd-macros`, `rstest-bdd-harness`) per ADR-005.
- Do not alter any public API surface in `rstest-bdd-harness`. The existing
  types (`HarnessAdapter`, `ScenarioRunner`, `ScenarioRunRequest`,
  `ScenarioMetadata`, `StdHarness`) are used as-is.
- Every new Rust module must begin with a `//!` module-level comment, and all
  public APIs must have Rustdoc comments with usage examples.
- Keep files under 400 lines by splitting modules when needed.
- Record design decisions in `docs/rstest-bdd-design.md`.
- Record user-facing usage in `docs/users-guide.md`.
- On completion, mark 9.2.2 as done in `docs/roadmap.md`.
- Quality gates must pass: `make check-fmt`, `make lint`, and `make test`.
- Use en-GB-oxendict spelling in comments and documentation.

## Tolerances (exception triggers)

- Scope: if delivery requires changing more than 25 files or more than 1500
  net lines of code, stop and escalate.
- Interfaces: if any existing public API in `rstest-bdd`, `rstest-bdd-macros`,
  or `rstest-bdd-harness` must be removed or made incompatible, stop and
  escalate.
- Dependencies: if a new external dependency (beyond existing workspace
  members) is needed in core crates, stop and escalate.
- Behaviour: if existing scenario tests regress, stop and escalate instead of
  weakening tests.
- Iterations: if the same failing gate (`check-fmt`, `lint`, or `test`) fails
  three times after attempted fixes, stop and escalate with logs.
- Ambiguity: if ADR-005 and current roadmap text conflict on interface shape,
  stop and request direction.
- File length: if any file exceeds 400 lines, split before proceeding.

## Risks

- Risk: the closure passed to `ScenarioRunner::new()` captures mutable local
  variables by move and must also reference inner-function item definitions
  (`__rstest_bdd_execute_single_step`, `__RstestBddScenarioReportGuard`) that
  are defined outside the closure. In Rust, inner item definitions (functions
  and structs defined inside a function body) are items visible via name
  resolution, not captured variables; they remain accessible inside a `move`
  closure in the same function body.
  Severity: low. Likelihood: low. Mitigation: verify with a prototype in
  Stage A.

- Risk: fallible scenarios return `Result<(), E>`. The `HarnessAdapter::run`
  method returns `T`. When `T = Result<(), E>`, the harness must propagate the
  `Err` faithfully. A custom harness that swallows errors would cause tests to
  pass silently.
  Severity: medium. Likelihood: low (harness-author responsibility).
  Mitigation: document in the user's guide that harness adapters must propagate
  the runner's return value.

- Risk: async scenarios combined with `harness` are not supported until phase
  9.3. If a user writes `async fn` with `harness = SomeHarness`, the generated
  code would call `HarnessAdapter::run()` (which is synchronous) from an async
  context, potentially producing confusing behaviour.
  Severity: medium. Likelihood: medium. Mitigation: emit a `compile_error!`
  when `harness` is specified and the scenario is async.

- Risk: the generated code references `rstest_bdd_harness::ScenarioRunner`,
  `ScenarioMetadata`, and `ScenarioRunRequest`. Users specifying `harness` must
  have `rstest-bdd-harness` in their dependency graph.
  Severity: medium. Likelihood: high. Mitigation: the integration tests in
  `rstest-bdd` already have the crate as a dev-dependency. Document the
  requirement in the user's guide.

- Risk: `Default` bound on harness types. The generated code instantiates the
  harness via `<HarnessType as Default>::default()`. This requires the harness
  type to implement `Default`. `StdHarness` already derives `Default`.
  Third-party harness types must also implement `Default`.
  Severity: low. Likelihood: low (most zero-config harnesses are naturally
  `Default`). Mitigation: add a `Default` compile-time assertion alongside the
  existing `HarnessAdapter` assertion and document the requirement.

## Progress

- [x] Baseline: verify `make test` passes.
- [x] Stage A: prototype and validate closure scoping. (Skipped; validated
  during plan phase -- item definitions stay outside closure by Rust scoping
  rules.)
- [x] Stage B: add `Default` assertion and async rejection.
- [x] Stage C: implement harness delegation codegen.
- [x] Stage D: update integration tests.
- [x] Stage E: add compile-fail tests.
- [x] Stage F: update documentation and roadmap.
- [x] Stage G: final quality gates.

## Surprises & discoveries

- Adding `Default` to the harness trait-bound assertion updated the error
  messages in existing compile-fail `.stderr` snapshots
  (`scenario_harness_invalid.stderr` and `scenarios_harness_invalid.stderr`).
  These now show both `HarnessAdapter` and `Default` unsatisfied bounds.
  Required `TRYBUILD=overwrite` to regenerate all affected snapshots.

- Stage A (closure scoping prototype) was unnecessary as a separate step.
  Rust item definitions (`fn`, `struct`, `const`, `static`) are visible by
  name resolution and do not participate in closure capture. This was validated
  during the plan phase and confirmed correct in generated output.

## Decision log

- Decision: require `Default` bound on harness types and instantiate with
  `<HarnessType as Default>::default()`.
  Rationale: proc macros cannot evaluate arbitrary expressions at expansion
  time. `Default::default()` is the cleanest zero-config instantiation pattern.
  `StdHarness` already derives `Default`. Third-party harnesses are expected to
  be zero-config or carry configuration via global/environment state. Adding a
  `HarnessFactory` trait or user-provided expression syntax would add
  complexity without clear benefit.
  Date/Author: 2026-02-15 / ExecPlan draft.

- Decision: emit `compile_error!` when `harness` is combined with `async fn`
  scenario signatures.
  Rationale: ADR-005 phases async harness support into 9.3 with
  `rstest-bdd-harness-tokio`. Allowing async + harness now would produce code
  that compiles but behaves incorrectly (calling synchronous
  `HarnessAdapter::run` from an async context). A clear compile error is better
  than silent misbehaviour.
  Date/Author: 2026-02-15 / ExecPlan draft.

- Decision: when `harness` is specified, wrap only the runtime execution
  portion in the closure; leave item definitions (inner `fn`, `struct`) outside.
  Rationale: inner item definitions are Rust items visible by name resolution,
  not captured variables. Placing them outside the closure keeps the generated
  code clean and avoids unnecessary boxing.
  Date/Author: 2026-02-15 / ExecPlan draft.

## Outcomes & retrospective

**Status: Complete.**

All quality gates pass: `make check-fmt`, `make lint`, `make test` (1166 Rust
tests + 47 Python tests). Roadmap entry 9.2.2 marked done.

### Files changed

- `crates/rstest-bdd-macros/src/codegen/scenario.rs` -- `Default` bound on
  harness trait assertion; async+harness rejection in both regular and outline
  code paths; `harness` field threaded through `ScenarioMetadata`.
- `crates/rstest-bdd-macros/src/codegen/scenario/runtime.rs` -- `harness()`
  method on `ScenarioTestConfig` trait; harness branching in
  `assemble_test_tokens_with_context`; module declaration for new `harness`
  submodule.
- `crates/rstest-bdd-macros/src/codegen/scenario/runtime/harness.rs` -- **new**;
  `assemble_test_tokens_with_harness()` extracted to keep `runtime.rs` under
  400-line limit.
- `crates/rstest-bdd-macros/src/codegen/scenario/runtime/types.rs` -- added
  `harness: Option<&'a syn::Path>` to codegen `ScenarioMetadata`.
- `crates/rstest-bdd-macros/src/codegen/scenario/tests.rs` -- unit test for
  `Default` bound in trait assertions.
- `crates/rstest-bdd/tests/scenario_harness.rs` -- integration tests:
  `RecordingHarness` delegation and `MetadataCapturingHarness` metadata
  verification.
- `crates/rstest-bdd/tests/trybuild_macros.rs` -- registered two new
  compile-fail fixtures.
- `crates/rstest-bdd/tests/fixtures_macros/scenario_harness_not_default.rs` --
  **new**; compile-fail: harness without `Default`.
- `crates/rstest-bdd/tests/fixtures_macros/scenario_harness_async_rejected.rs`
  -- **new**; compile-fail: harness + async fn.
- `crates/rstest-bdd/tests/fixtures_macros/scenario_harness_not_default.stderr`,
  `scenario_harness_async_rejected.stderr` -- **new**; expected error snapshots.
- `crates/rstest-bdd/tests/fixtures_macros/scenario_harness_invalid.stderr`,
  `scenarios_harness_invalid.stderr` -- updated to include `Default` bound
  error.
- `docs/users-guide.md` -- documented custom harness usage, `Default`
  requirement, async limitation.
- `docs/rstest-bdd-design.md` -- updated section 2.7.3 with delegation design.
- `docs/roadmap.md` -- marked 9.2.2 as done.

### What went well

- The harness delegation is a clean, separate code path activated only when
  `harness` is `Some`, preserving full backward compatibility.
- The `Default::default()` instantiation pattern avoids proc-macro expression
  evaluation complexity.
- Compile-time async rejection prevents silent runtime misbehaviour.

### What to watch

- `assemble_test_tokens_with_harness` closely mirrors `assemble_test_tokens`.
  If the non-harness path changes, the harness path must be updated in tandem.
  Consider unifying them in a future refactor.
- Phase 9.3 (async harness via `rstest-bdd-harness-tokio`) will need to lift
  the async rejection and introduce an async-aware `HarnessAdapter` variant.

## Context and orientation

The `rstest-bdd` workspace implements a behaviour-driven development (BDD)
framework for Rust built on `rstest`. Two procedural macros drive test
generation:

1. `#[scenario(path = "...", ...)]` -- an attribute macro that reads a Gherkin
   `.feature` file at compile time and generates an `#[rstest::rstest]`-backed
   test function. Entry point:
   `crates/rstest-bdd-macros/src/macros/scenario/mod.rs`.

2. `scenarios!("dir", ...)` -- a function-like macro that recursively discovers
   `.feature` files and generates a module with one test per scenario. Entry
   point: `crates/rstest-bdd-macros/src/macros/scenarios/mod.rs`.

Key files in the code generation pipeline:

- `crates/rstest-bdd-macros/src/codegen/scenario.rs` -- central code
  generation orchestrator. Contains `ScenarioConfig` (line 43) with `harness`
  and `attributes` fields, `generate_scenario_code()` (line 181),
  `generate_regular_scenario_code()` (line 204),
  `generate_outline_scenario_code()` (line 257), `generate_test_attrs()` (line
  98), and `generate_trait_assertions()` (line 132).

- `crates/rstest-bdd-macros/src/codegen/scenario/runtime.rs` -- runtime
  scaffolding generator. Contains `assemble_test_tokens()` (line 219) which
  produces the test body token stream, and
  `assemble_test_tokens_with_context()` (line 287) which is the common dispatch
  function. Also contains the `ScenarioTestConfig` trait (line 34) implemented
  by `TestTokensConfig` and `OutlineTestTokensConfig`.

- `crates/rstest-bdd-macros/src/codegen/scenario/runtime/types.rs` -- shared
  data structures. Contains `ScenarioMetadata<'a>` (line 18, the codegen
  metadata struct, distinct from the harness crate's `ScenarioMetadata`),
  `TestTokensConfig` (line 44), `CodeComponents` (line 68),
  `TokenAssemblyContext` (line 77), and `ScenarioLiteralsInput` (line 59).

- `crates/rstest-bdd-macros/src/codegen/scenario/runtime/body.rs` --
  `wrap_scenario_block()` that wraps the user's scenario body for
  fallible/async scenarios.

- `crates/rstest-bdd-macros/src/codegen/scenario/runtime/generators/` --
  individual generators for step executors, loops, guards, and skip handlers.

- `crates/rstest-bdd-macros/src/codegen/mod.rs` -- crate path helpers.
  `rstest_bdd_path()` and `rstest_bdd_harness_path()` resolve qualified paths
  for code generation.

The harness crate (`crates/rstest-bdd-harness/`) provides:

- `HarnessAdapter` trait (`src/adapter.rs`):
  `fn run<T>(&self, request: ScenarioRunRequest<'_, T>) -> T`
- `ScenarioRunner<'a, T>` (`src/runner.rs`): wraps `Box<dyn FnOnce() -> T +
  'a>`; constructed via `ScenarioRunner::new(closure)`.
- `ScenarioRunRequest<'a, T>` (`src/runner.rs`): bundles `ScenarioMetadata` +
  `ScenarioRunner`; constructed via `ScenarioRunRequest::new(metadata, runner)`.
- `ScenarioMetadata` (`src/runner.rs`): `new(feature_path: impl Into<String>,
  scenario_name: impl Into<String>, scenario_line: u32, tags: Vec<String>)`.
- `StdHarness` (`src/std_harness.rs`): `#[derive(Default)]`, implements
  `HarnessAdapter` by calling `request.run()` directly.

Existing tests for harness parameters:

- `crates/rstest-bdd/tests/scenario_harness.rs` -- three integration tests
  verifying that `#[scenario]` accepts `harness`, `attributes`, and both
  together. Uses `StdHarness` and `DefaultAttributePolicy` with
  `web_search.feature`.
- `crates/rstest-bdd/tests/fixtures_macros/scenario_harness_invalid.rs` and
  `scenario_attributes_invalid.rs` -- trybuild compile-fail fixtures.
- `crates/rstest-bdd-harness/tests/harness_behaviour.rs` -- unit-level
  behavioural tests for the harness adapter primitives.

The current generated test body (from `assemble_test_tokens`, line 247 of
`runtime.rs`) looks like this (simplified):

    const __RSTEST_BDD_FEATURE_PATH: &str = "...";
    const __RSTEST_BDD_SCENARIO_NAME: &str = "...";
    const __RSTEST_BDD_SCENARIO_LINE: u32 = ...;
    static __RSTEST_BDD_SCENARIO_TAGS: LazyLock<...> = ...;

    fn __rstest_bdd_execute_single_step(...) { ... }   // step_executor
    fn __rstest_bdd_extract_skip_message(...) { ... }   // skip_extractor
    struct __RstestBddScenarioReportGuard { ... }       // scenario_guard

    let __rstest_bdd_allow_skipped: bool = ...;
    // ctx_prelude (fixture setup)
    let mut ctx = { ... };                              // ctx_inserts
    let mut __rstest_bdd_scenario_guard = __RstestBddScenarioReportGuard::new(...);
    let mut __rstest_bdd_skipped: Option<Option<String>> = None;
    let mut __rstest_bdd_skipped_at: Option<usize> = None;
    // step_executor_loop
    // skip_handler
    // ctx_postlude
    // user block

When `harness` is specified, the portion from `let __rstest_bdd_allow_skipped`
downward is wrapped in a closure and passed to the harness adapter. The item
definitions (constants, functions, structs) remain outside the closure because
they are Rust items, not captured variables.

## Plan of work

### Stage A: baseline and prototype validation (no production code changes)

Run `make test` to confirm the workspace is green. Mentally validate that inner
`fn` and `struct` definitions are accessible from within a `move` closure in the
same function body. Rust's name resolution rules guarantee this: item
definitions in a function body are items, not local bindings, and are visible
to any code in the same function scope including closures.

Go/no-go: baseline `make test` exits 0.

### Stage B: add `Default` assertion and async rejection

Two changes in `crates/rstest-bdd-macros/src/codegen/scenario.rs`:

1. In `generate_trait_assertions()` (line 132): when `harness` is
   `Some(harness_path)`, add `+ Default` to the existing trait bound so the
   emitted assertion becomes:

       const _: () = {
           fn __assert_harness<T: #harness_crate::HarnessAdapter + Default>() {}
           fn __call() { __assert_harness::<#harness_path>(); }
       };

   This merges both bounds into a single assertion function. The existing
   `AttributePolicy` assertion is unchanged.

2. In `generate_regular_scenario_code()` (line 204) and
   `generate_outline_scenario_code()` (line 257): add an early check that
   rejects `harness` combined with async scenarios. If
   `config.harness.is_some() && config.runtime.is_async()`, emit a
   `compile_error!`.

3. In `crates/rstest-bdd-macros/src/codegen/scenario/tests.rs`: add a test
   `trait_assertions_harness_includes_default_bound` that verifies the
   generated assertion tokens contain `Default` when harness is specified.

Go/no-go: `cargo test -p rstest-bdd-macros` passes. All existing tests remain
green.

### Stage C: implement harness delegation codegen (core change)

This is the main stage, confined to the codegen pipeline.

Step C.1: thread harness into runtime types.

In `crates/rstest-bdd-macros/src/codegen/scenario/runtime/types.rs`: add
`pub(crate) harness: Option<&'a syn::Path>` to `ScenarioMetadata<'a>`.

In `crates/rstest-bdd-macros/src/codegen/scenario.rs`: in
`generate_regular_scenario_code()` and `generate_outline_scenario_code()`, add
`harness: config.harness` when constructing `ScenarioMetadata`.

Step C.2: extend `ScenarioTestConfig` trait.

In `crates/rstest-bdd-macros/src/codegen/scenario/runtime.rs`: add
`fn harness(&self) -> Option<&syn::Path>` to the `ScenarioTestConfig` trait.
Implement for `TestTokensConfig` and `OutlineTestTokensConfig`.

Step C.3: create `assemble_test_tokens_with_harness()`.

In `crates/rstest-bdd-macros/src/codegen/scenario/runtime.rs`, add a new
function that generates the harness-delegated test body. Item definitions stay
outside the closure; runtime execution goes inside.

Step C.4: modify dispatch.

In `assemble_test_tokens_with_context()`, add `harness` parameter and branch:
harness present calls `assemble_test_tokens_with_harness()`; harness absent
calls existing `assemble_test_tokens()` unchanged.

Go/no-go: `cargo check -p rstest-bdd-macros` succeeds. `cargo test -p
rstest-bdd-macros` passes.

### Stage D: update integration tests

Add `RecordingHarness` and `MetadataCapturingHarness` tests to
`crates/rstest-bdd/tests/scenario_harness.rs` to prove delegation.

Go/no-go: `cargo test -p rstest-bdd` passes.

### Stage E: add compile-fail tests

Add trybuild fixtures for: harness without `Default`; harness with `async fn`.

Go/no-go: `cargo test -p rstest-bdd` passes.

### Stage F: update documentation and roadmap

Update `docs/users-guide.md`, `docs/rstest-bdd-design.md`, `docs/roadmap.md`.

### Stage G: final quality gates

Run `make check-fmt`, `make lint`, and `make test`.

Go/no-go: all three exit with status 0.

## Concrete steps

All commands run from the repository root (`/home/user/project`).

1. Baseline:

       set -o pipefail
       make test 2>&1 | tee /tmp/9-2-2-baseline-test.log

2. Stage B: edit `codegen/scenario.rs` and `codegen/scenario/tests.rs`. Verify:

       cargo test -p rstest-bdd-macros 2>&1 | tee /tmp/9-2-2-stage-b.log

3. Stage C: edit `runtime/types.rs`, `codegen/scenario.rs`, `runtime.rs`.
   Verify:

       cargo check -p rstest-bdd-macros
       cargo test -p rstest-bdd-macros 2>&1 | tee /tmp/9-2-2-stage-c.log

4. Stage D: edit `tests/scenario_harness.rs`. Verify:

       cargo test -p rstest-bdd 2>&1 | tee /tmp/9-2-2-stage-d.log

5. Stage E: create compile-fail fixtures. Verify:

       cargo test -p rstest-bdd 2>&1 | tee /tmp/9-2-2-stage-e.log

6. Stage F: edit docs and roadmap.

7. Stage G:

       set -o pipefail; make check-fmt 2>&1 | tee /tmp/9-2-2-check-fmt.log
       set -o pipefail; make lint 2>&1 | tee /tmp/9-2-2-lint.log
       set -o pipefail; make test 2>&1 | tee /tmp/9-2-2-test.log

## Validation and acceptance

- When `harness = SomeHarness` is specified, the generated test body delegates
  execution to `<SomeHarness as Default>::default().run(request)`.
- A custom `RecordingHarness` integration test proves the harness `run()`
  method is called exactly once and receives correct metadata.
- `StdHarness` delegation produces identical test outcomes to the non-harness
  path.
- Combining `harness` with `async fn` produces a clear `compile_error!`.
- A harness type not implementing `Default` produces a clear compile error.
- When `harness` is omitted, generated code is unchanged.
- `make check-fmt`, `make lint`, and `make test` all exit with status 0.

## Idempotence and recovery

Most steps are repeatable. If a gate fails, inspect the corresponding
`/tmp/9-2-2-*.log`, fix the smallest local cause, re-run the failed command,
and re-run the full gates at the end.

## Artifacts and notes

Expected evidence files:

- `/tmp/9-2-2-baseline-test.log`
- `/tmp/9-2-2-stage-b.log`
- `/tmp/9-2-2-stage-c.log`
- `/tmp/9-2-2-stage-d.log`
- `/tmp/9-2-2-stage-e.log`
- `/tmp/9-2-2-check-fmt.log`
- `/tmp/9-2-2-lint.log`
- `/tmp/9-2-2-test.log`

## Interfaces and dependencies

Generated code when `harness` is specified references these types from
`rstest-bdd-harness`:

- `rstest_bdd_harness::ScenarioMetadata::new(feature_path, scenario_name,
  line, tags)`
- `rstest_bdd_harness::ScenarioRunner::new(closure)`
- `rstest_bdd_harness::ScenarioRunRequest::new(metadata, runner)`
- `<HarnessType as Default>::default()`
- `harness.run(request)`

Updated compile-time assertion:

    const _: () = {
        fn __assert_harness<T: rstest_bdd_harness::HarnessAdapter + Default>() {}
        fn __call() { __assert_harness::<#harness_path>(); }
    };

New trait method on `ScenarioTestConfig` (internal):

    fn harness(&self) -> Option<&syn::Path>;

New field on codegen `ScenarioMetadata<'a>` (internal):

    pub(crate) harness: Option<&'a syn::Path>,

No changes to public APIs in `rstest-bdd-harness`.

## Revision note

Initial draft created from roadmap phase 9.2.2, ADR-005 harness decision,
ExecPlan 9.2.1 (completed), and thorough codebase exploration.
