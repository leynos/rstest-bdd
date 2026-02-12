# ExecPlan 9.2.1: Extend `#[scenario]` and `scenarios!` with harness and attribute policy parameters

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

`PLANS.md` is not present in the repository at the time of writing, so this
ExecPlan is the governing plan for this task.

## Purpose / big picture

Phase 9.1 introduced the framework-agnostic harness foundation in
`rstest-bdd-harness` (the `HarnessAdapter` trait, `AttributePolicy` trait,
`StdHarness`, and `DefaultAttributePolicy`). These contracts exist but are not
yet accessible from the macro layer. After this work, users of `#[scenario]`
and `scenarios!` can specify a harness adapter and attribute policy via new
optional parameters:

    #[scenario(
        path = "tests/features/demo.feature",
        harness = rstest_bdd_harness::StdHarness,
        attributes = rstest_bdd_harness::DefaultAttributePolicy,
    )]
    fn my_test() {}

    scenarios!(
        "tests/features/auto",
        harness = rstest_bdd_harness::StdHarness,
        attributes = rstest_bdd_harness::DefaultAttributePolicy,
    );

Success is observable when:

- Both macros accept `harness = <path>` and `attributes = <path>` without
  errors.
- Generated code includes compile-time trait-bound assertions that reject
  types not implementing `HarnessAdapter` or `AttributePolicy`.
- Omitting both parameters preserves exact backward compatibility.
- Unit tests, integration tests, and trybuild compile-fail tests pass.
- `make check-fmt`, `make lint`, and `make test` all succeed.
- Documentation is updated and roadmap entry 9.2.1 is marked done.

## Constraints

- Implement only the 9.2.1 scope from `docs/roadmap.md`: parse and validate
  the new macro arguments, thread them through the code generation pipeline,
  and emit compile-time trait-bound assertions. Do not implement 9.2.2
  (execution delegation) or 9.2.3 (runtime compatibility alias) in this change.
- Keep Tokio and GPUI dependencies out of core crates (`rstest-bdd`,
  `rstest-bdd-macros`, `rstest-bdd-harness`) per ADR-005.
- Preserve existing public behaviour for current users: all existing macro
  invocations without `harness`/`attributes` must produce identical output.
- The `runtime = "tokio-current-thread"` parameter in `scenarios!` continues
  to work unchanged.
- Every new Rust module must begin with a `//!` module-level comment, and all
  public APIs must have Rustdoc comments with usage examples.
- Keep files under 400 lines by splitting modules when needed.
- Record design decisions in `docs/rstest-bdd-design.md`.
- Record user-facing usage in `docs/users-guide.md`.
- On completion, mark 9.2.1 as done in `docs/roadmap.md`.
- Quality gates must pass before completion: `make check-fmt`, `make lint`,
  and `make test`.

## Tolerances (exception triggers)

- Scope: if delivery requires changing more than 25 files or more than 1200
  net lines of code (LOC), stop and escalate.
- Interfaces: if any existing public API in `rstest-bdd` or
  `rstest-bdd-macros` must be removed or made incompatible, stop and escalate.
- Dependencies: if a new external dependency (beyond `rstest-bdd-harness`)
  is needed in core crates, stop and escalate.
- Behaviour: if existing scenario tests regress, stop and escalate instead of
  weakening tests.
- Iterations: if the same failing gate (`check-fmt`, `lint`, or `test`) fails
  three times after attempted fixes, stop and escalate with logs.
- Ambiguity: if ADR-005 and current roadmap text conflict on interface shape,
  stop and request direction before coding further.

## Risks

- Risk: proc macros cannot call user-defined trait methods at expansion time,
  so `AttributePolicy::test_attributes()` cannot be evaluated for arbitrary
  user types during macro expansion.
  Severity: medium
  Likelihood: certain (inherent Rust proc-macro limitation)
  Mitigation: emit compile-time const trait-bound assertions to validate the
  type. Defer full attribute-driven codegen to 9.2.2. For 9.2.1, when
  `attributes` is specified, emit only `#[rstest::rstest]` (always required)
  and skip the `RuntimeMode`-based tokio attribute generation.

- Risk: adding `rstest-bdd-harness` as a dependency of the proc-macro crate
  could introduce unwanted transitive dependencies.
  Severity: low
  Likelihood: low
  Mitigation: `rstest-bdd-harness` is dependency-light by ADR-005 design.
  Verify with `cargo tree -p rstest-bdd-macros` after wiring.

- Risk: trybuild snapshot files may need updating if error messages change
  across Rust compiler versions.
  Severity: low
  Likelihood: low
  Mitigation: use compile-fail tests that check for the presence of key trait
  names in the error output rather than exact compiler messages.

- Risk: threading two new `Option<syn::Path>` fields through the pipeline
  touches many structs and call sites, increasing the risk of a missed
  connection.
  Severity: medium
  Likelihood: medium
  Mitigation: implement threading in a single stage, verify with
  `cargo check -p rstest-bdd-macros` immediately, and test that existing tests
  still pass before moving to code generation changes.

## Progress

- [x] (2026-02-10 00:00Z) Explored codebase: macro implementation, harness
      crate, codegen pipeline, existing tests, and documentation.
- [x] (2026-02-10 00:10Z) Drafted this ExecPlan.
- [x] (2026-02-10 00:15Z) Baseline `make test` passed (1138 tests).
- [x] (2026-02-10 00:20Z) Stage B: Added `rstest-bdd-harness` dependency and
      `rstest_bdd_harness_path()` helper.
- [x] (2026-02-10 00:30Z) Stage C: Extended `#[scenario]` argument parsing with
      `harness` and `attributes` params and unit tests.
- [x] (2026-02-10 00:40Z) Stage D: Extended `scenarios!` argument parsing with
      `harness` and `attributes` params and unit tests.
- [x] (2026-02-10 00:50Z) Stage E: Threaded new parameters through pipeline
      (`ScenarioConfig`, `try_scenario`, `FeatureProcessingContext`,
      `ScenarioTestContext`, `generate_scenario_test`).
- [x] (2026-02-10 01:00Z) Stage F: Updated code generation to emit trait-bound
      const assertions via `generate_trait_assertions`.
- [x] (2026-02-10 01:10Z) Stage G: Added integration tests
      (`scenario_harness.rs`) and trybuild compile-pass fixture
      (`scenario_harness_params.rs`).
- [x] (2026-02-10 01:15Z) Stage H: Added trybuild compile-fail fixtures
      (`scenario_harness_invalid.rs`, `scenario_attributes_invalid.rs`) with
      generated `.stderr` snapshots.
- [x] (2026-02-10 01:20Z) Stage I: Updated documentation (users guide, design
      doc, roadmap).
- [x] (2026-02-10 01:30Z) Stage J: All quality gates passed. `make check-fmt`,
      `make lint`, and `make test` (1163 tests + 47 Python tests) all exit 0.

## Surprises & discoveries

- Observation: const assertions emitted alongside `#[rstest::rstest]` as
  "test attributes" caused `expected fn` compile errors because `const _: ()`
  cannot appear in attribute position.
  Evidence: integration test `scenario_harness.rs` failed with "expected `fn`"
  when const blocks were placed before the `#[scenario]` function.
  Impact: split `generate_test_attrs` into two functions: `generate_test_attrs`
  (only `#[rstest::rstest]` and optional `#[tokio::test]`) and
  `generate_trait_assertions` (const blocks emitted as sibling items).

## Decision log

- Decision: emit compile-time trait-bound assertions rather than trying to
  evaluate `AttributePolicy::test_attributes()` at macro expansion time.
  Rationale: Rust proc macros cannot call arbitrary trait methods from user
  crates during expansion. Const assertions verify the type implements the
  trait and produce clear compiler errors. Full attribute evaluation is deferred
  to 9.2.2 (delegation).
  Date/Author: 2026-02-10 / Codex

- Decision: when `attributes = SomePolicy` is specified, emit only
  `#[rstest::rstest]` and skip the `RuntimeMode`-based `#[tokio::test]`
  generation. Rationale: the attribute policy is the new extension point for
  controlling test attributes per ADR-005. When a user specifies a policy,
  they are opting into the new system and the macro should not second-guess
  the policy by also emitting framework-specific attributes. The user's policy
  (or manual `#[tokio::test]` annotation) is trusted.
  Date/Author: 2026-02-10 / Codex

- Decision: add `rstest-bdd-harness` as a compile-time dependency of
  `rstest-bdd-macros` to reference `HarnessAdapter` and `AttributePolicy`
  trait paths in const assertions. Rationale: `rstest-bdd-harness` is
  dependency-light per ADR-005 and this enables the macro to emit
  well-qualified trait paths.
  Date/Author: 2026-02-10 / Codex

- Decision: both `harness` and `attributes` parameters accept `syn::Path`
  values (Rust type paths). Rationale: users can reference types from any
  crate, enabling extensibility by third-party harness crates.
  Date/Author: 2026-02-10 / Codex

## Outcomes & retrospective

All acceptance criteria have been met:

- Both `#[scenario]` and `scenarios!` accept `harness = <path>` and
  `attributes = <path>` optional parameters. Omitting them preserves exact
  backward compatibility (all 1163 existing tests pass unchanged).
- Generated code includes const trait-bound assertions when either parameter is
  supplied.
- Types not implementing `HarnessAdapter` or `AttributePolicy` produce clear
  compile errors (verified by trybuild compile-fail fixtures).
- When `attributes` is specified, the macro emits only `#[rstest::rstest]` and
  skips `RuntimeMode`-based tokio attribute generation.
- Unit tests cover argument parsing for both macros (8 new tests each).
- Integration tests verify end-to-end scenario execution with the new params
  (3 integration tests in `scenario_harness.rs`).
- Documentation updated: users guide, design doc, roadmap.
- All quality gates pass: `make check-fmt`, `make lint`, `make test`.

Lessons learned:

1. **Const assertions cannot go in attribute position.** The initial approach of
   emitting const trait-bound assertions alongside `#[rstest::rstest]` in the
   test attribute token stream failed because `const _: () = { ... };` is a
   statement/item, not an attribute. The fix was straightforward: split into
   `generate_test_attrs` (attributes only) and `generate_trait_assertions`
   (sibling items emitted before the test function).

2. **Lazy crate path resolution matters.** The `rstest_bdd_harness_path()`
   helper must only be called when the harness crate is actually needed.
   Downstream crates that don't depend on `rstest-bdd-harness` would panic if
   the helper was called unconditionally. An early return guard fixed this.

3. **File length limits require planning.** Adding doc comments and fields to
   `test_generation.rs` pushed it over the 400-line limit. Compacting doc
   comments rather than splitting the module was the right trade-off here since
   the struct and function are tightly coupled.

## Context and orientation

The `rstest-bdd` workspace implements a BDD framework for Rust built on
`rstest`. Two procedural macros drive test generation:

1. `#[scenario(path = "...", ...)]` — an attribute macro that reads a Gherkin
   `.feature` file at compile time and generates an `#[rstest::rstest]`-backed
   test function that runs the scenario's steps in order.

2. `scenarios!("dir", ...)` — a function-like macro that recursively discovers
   `.feature` files and generates a module with one test per scenario.

Key files in the macro crate (`crates/rstest-bdd-macros/`):

- `src/macros/scenario/args.rs` — parses `#[scenario]` arguments into
  `ScenarioArgs { path, selector, tag_filter }`.
- `src/macros/scenario/mod.rs` — orchestrates parsing, validation, and code
  generation for `#[scenario]`.
- `src/macros/scenarios/macro_args.rs` — parses `scenarios!` arguments into
  `ScenariosArgs { dir, tag_filter, fixtures, runtime }`.
- `src/macros/scenarios/mod.rs` — orchestrates `scenarios!` expansion.
- `src/macros/scenarios/test_generation.rs` — generates individual test
  functions from discovered scenarios; uses `ScenarioTestContext`.
- `src/codegen/scenario.rs` — central code generation: `ScenarioConfig`,
  `generate_test_attrs`, `generate_scenario_code`.
- `src/codegen/mod.rs` — `rstest_bdd_path()` helper that resolves the runtime
  crate path for generated code.

The harness crate (`crates/rstest-bdd-harness/`) provides:

- `HarnessAdapter` trait —
  `fn run<T>(&self, request: ScenarioRunRequest<'_, T>) -> T`
- `AttributePolicy` trait — `fn test_attributes() -> &'static [TestAttribute]`
- `StdHarness` — default synchronous pass-through harness
- `DefaultAttributePolicy` — emits only `#[rstest::rstest]`

The policy crate (`crates/rstest-bdd-policy/`) provides `RuntimeMode` and
`TestAttributeHint` enums used by the existing `generate_test_attrs` function.

## Plan of work

### Stage A: Baseline (no code changes)

Run `make test` and confirm the workspace is green before any edits.

Go/no-go: baseline `make test` exits with status 0.

### Stage B: Dependency wiring

Add `rstest-bdd-harness` as a dependency of `rstest-bdd-macros` so the macro
crate can reference `HarnessAdapter` and `AttributePolicy` trait paths. Add a
`rstest_bdd_harness_path()` helper in `src/codegen/mod.rs` that mirrors the
existing `rstest_bdd_path()` pattern using `proc_macro_crate::crate_name`.

Go/no-go: `cargo check -p rstest-bdd-macros` succeeds.

### Stage C: `#[scenario]` argument parsing

In `src/macros/scenario/args.rs`:

- Add `Harness(syn::Path)` and `Attributes(syn::Path)` variants to the
  `ScenarioArg` enum, parsing `harness = <path>` and `attributes = <path>`.
- Add `harness: Option<syn::Path>` and `attributes: Option<syn::Path>` fields
  to `ScenarioArgs`.
- Enforce no duplicates, update error messages.
- Add unit tests for the new arguments: parse each alone, together, with
  existing args, reject duplicates.

Go/no-go: `cargo test -p rstest-bdd-macros` passes.

### Stage D: `scenarios!` argument parsing

In `src/macros/scenarios/macro_args.rs`:

- Add `Harness(syn::Path)` and `Attributes(syn::Path)` variants to the
  `ScenariosArg` enum, parsing `harness = <path>` and `attributes = <path>`.
- Add `harness: Option<syn::Path>` and `attributes: Option<syn::Path>` fields
  to `ScenariosArgs`.
- Enforce no duplicates, update error messages.
- Add unit tests for the new arguments.

Go/no-go: `cargo test -p rstest-bdd-macros` passes.

### Stage E: Pipeline threading

Thread the new `harness` and `attributes` fields through the code generation
pipeline:

1. Add `harness: Option<&'a syn::Path>` and `attributes: Option<&'a syn::Path>`
   to `ScenarioConfig` in `src/codegen/scenario.rs`.

2. In `src/macros/scenario/mod.rs` (`try_scenario`): extract `harness` and
   `attributes` from `ScenarioArgs` and pass to `ScenarioConfig`.

3. In `src/macros/scenarios/mod.rs`: add `harness` and `attributes` to
   `FeatureProcessingContext` and thread from `ScenariosArgs`.

4. In `src/macros/scenarios/test_generation.rs`: add `harness` and `attributes`
   to `ScenarioTestContext` and pass to `ScenarioConfig` in
   `generate_scenario_test`.

Go/no-go: `cargo test -p rstest-bdd-macros` passes; all existing tests green.

### Stage F: Code generation — trait assertions

Modify `generate_test_attrs` in `src/codegen/scenario.rs` to accept the new
parameters. When `attributes` is `Some(policy_path)`, generate:

1. A const assertion verifying the policy implements `AttributePolicy`:

       const _: () = {
           fn __assert_policy<T: rstest_bdd_harness::AttributePolicy>() {}
           __assert_policy::<#policy_path>();
       };

2. Only `#[rstest::rstest]` as the test attribute (skip `RuntimeMode`-based
   tokio attribute generation).

When `harness` is `Some(harness_path)`, generate a const assertion verifying
the harness implements `HarnessAdapter`:

    const _: () = {
        fn __assert_harness<T: rstest_bdd_harness::HarnessAdapter>() {}
        __assert_harness::<#harness_path>();
    };

The assertions are emitted inside the generated test function body (alongside
the existing scenario scaffolding) so they participate in the same compilation
unit as the user's types.

When neither parameter is specified, the existing behavior is unchanged.

Update all call sites of `generate_test_attrs`:

- `generate_regular_scenario_code`
- `generate_outline_scenario_code`

Add codegen unit tests in `src/codegen/scenario/tests.rs` verifying:

- With `attributes` specified: output contains `rstest :: rstest` but not
  `tokio :: test`.
- With `harness` specified: const assertion token is present.
- Without either: existing behaviour preserved.

Go/no-go: `cargo test -p rstest-bdd-macros` passes.

### Stage G: Integration and behavioural tests

Add integration tests in `crates/rstest-bdd/tests/` that exercise the new
parameters end-to-end:

- `#[scenario]` with `harness = rstest_bdd_harness::StdHarness` — scenario
  runs normally.
- `#[scenario]` with `attributes = rstest_bdd_harness::DefaultAttributePolicy`
  — scenario runs normally.
- `#[scenario]` with both — scenario runs normally.
- `scenarios!` with both — discovered scenarios run normally.

These tests reuse existing `.feature` files where possible.

Go/no-go: `cargo test -p rstest-bdd` passes.

### Stage H: Trybuild compile-fail tests

Add trybuild fixture(s) in `crates/rstest-bdd/tests/fixtures_macros/` that
verify compile-time rejection of types not implementing the required traits:

- `harness = SomeStructThatIsNotAHarness` → compile error mentioning
  `HarnessAdapter`.
- `attributes = SomeStructThatIsNotAPolicy` → compile error mentioning
  `AttributePolicy`.

Go/no-go: `cargo test -p rstest-bdd` passes (trybuild assertions hold).

### Stage I: Documentation

1. Update `docs/users-guide.md`: add entries for `harness` and `attributes` in
   the "Binding tests to scenarios" table, add code examples, and note that
   execution delegation is coming in a future phase.

2. Update `docs/rstest-bdd-design.md`: record the macro integration design
   decisions (proc-macro constraint, const assertion approach, policy trust
   model).

3. Mark 9.2.1 as done in `docs/roadmap.md`: change `- [ ] 9.2.1.` to
   `- [x] 9.2.1.`.

Go/no-go: `make markdownlint` passes (if available), doc examples are correct.

### Stage J: Final quality gates

Run the required quality gates and capture output:

    set -o pipefail; make check-fmt 2>&1 | tee /tmp/9-2-1-check-fmt.log
    set -o pipefail; make lint 2>&1 | tee /tmp/9-2-1-lint.log
    set -o pipefail; make test 2>&1 | tee /tmp/9-2-1-test.log

Go/no-go: all three exit with status 0.

## Concrete steps

All commands run from the repository root (`/home/user/project`).

1. Baseline:

       set -o pipefail
       make test 2>&1 | tee /tmp/9-2-1-baseline-test.log

2. Add dependency (Stage B):

   Edit `crates/rstest-bdd-macros/Cargo.toml`, add under `[dependencies]`:

       rstest-bdd-harness.workspace = true

   Add `rstest_bdd_harness_path()` to `crates/rstest-bdd-macros/src/codegen/mod.rs`.

       cargo check -p rstest-bdd-macros

3. Argument parsing (Stages C–D):

   Edit `args.rs` and `macro_args.rs` as described.

       cargo test -p rstest-bdd-macros 2>&1 | tee /tmp/9-2-1-parsing-tests.log

4. Pipeline threading (Stage E):

   Edit `codegen/scenario.rs`, `scenario/mod.rs`, `scenarios/mod.rs`,
   `test_generation.rs` as described.

       cargo test -p rstest-bdd-macros 2>&1 | tee /tmp/9-2-1-threading-tests.log

5. Code generation (Stage F):

   Edit `codegen/scenario.rs` and add tests.

       cargo test -p rstest-bdd-macros 2>&1 | tee /tmp/9-2-1-codegen-tests.log

6. Integration tests (Stages G–H):

       cargo test -p rstest-bdd 2>&1 | tee /tmp/9-2-1-integration-tests.log

7. Documentation (Stage I):

   Edit `docs/users-guide.md`, `docs/rstest-bdd-design.md`, `docs/roadmap.md`.

8. Final gates (Stage J):

       set -o pipefail; make check-fmt 2>&1 | tee /tmp/9-2-1-check-fmt.log
       set -o pipefail; make lint 2>&1 | tee /tmp/9-2-1-lint.log
       set -o pipefail; make test 2>&1 | tee /tmp/9-2-1-test.log

## Validation and acceptance

Acceptance criteria for 9.2.1:

- `#[scenario]` accepts `harness = <path>` and `attributes = <path>` optional
  parameters; omitting them preserves existing behaviour.
- `scenarios!` accepts the same two optional parameters.
- Generated code includes const trait-bound assertions when parameters are
  supplied.
- Types not implementing `HarnessAdapter` or `AttributePolicy` produce clear
  compile errors.
- When `attributes` is specified, the macro emits `#[rstest::rstest]` only
  (no `RuntimeMode`-based tokio attribute).
- Unit tests cover argument parsing for both macros.
- Integration tests verify end-to-end scenario execution with the new params.
- Trybuild compile-fail tests verify rejection of invalid types.
- `docs/users-guide.md` documents the new parameters with examples.
- `docs/rstest-bdd-design.md` records design decisions.
- `docs/roadmap.md` marks 9.2.1 as `[x]`.
- `make check-fmt`, `make lint`, and `make test` all exit with status 0.

## Idempotence and recovery

Most steps are repeatable. If a gate fails:

- Inspect the corresponding `/tmp/9-2-1-*.log`.
- Fix the smallest local cause.
- Re-run only the failed command.
- Re-run the full required gates at the end.

If interface design needs to change after Stage E, record the change in the
Decision Log, update this plan, and continue.

## Artifacts and notes

Expected evidence files:

- `/tmp/9-2-1-baseline-test.log`
- `/tmp/9-2-1-parsing-tests.log`
- `/tmp/9-2-1-threading-tests.log`
- `/tmp/9-2-1-codegen-tests.log`
- `/tmp/9-2-1-integration-tests.log`
- `/tmp/9-2-1-check-fmt.log`
- `/tmp/9-2-1-lint.log`
- `/tmp/9-2-1-test.log`

## Interfaces and dependencies

New macro argument surface for `#[scenario]`:

    #[scenario(
        path = "features/demo.feature",
        harness = some::HarnessType,        // optional
        attributes = some::PolicyType,       // optional
    )]
    fn my_test() {}

New macro argument surface for `scenarios!`:

    scenarios!(
        "tests/features/auto",
        harness = some::HarnessType,        // optional
        attributes = some::PolicyType,       // optional
    );

The `harness` parameter accepts any `syn::Path` to a type that implements
`rstest_bdd_harness::HarnessAdapter`. The `attributes` parameter accepts any
`syn::Path` to a type that implements `rstest_bdd_harness::AttributePolicy`.

Both parameters may be combined freely with existing parameters (`path`,
`index`, `name`, `tags`, `dir`, `fixtures`, `runtime`).

Dependency addition: `rstest-bdd-macros` gains a compile-time dependency on
`rstest-bdd-harness` (workspace member, no external dependencies).

## Revision note

Initial draft created from roadmap phase 9.2.1, ADR-005 harness decision,
and thorough codebase exploration of macro argument parsing, code generation
pipeline, and harness crate interfaces.

Revision (2026-02-10): Marked COMPLETE. All stages A–J finished. Added
outcomes, retrospective, and lessons learned. Quality gates all pass.
