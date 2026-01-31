# Enable fallible scenario bodies for #[scenario]

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

PLANS.md is not present in this repository.

## Purpose / big picture

After this change, `#[scenario]` tests can return `Result<(), E>` or
`StepResult<(), E>` so scenario bodies can use `?` without extra boilerplate.
When a step requests a skip, the generated test should return `Ok(())` for
fallible scenarios, and when a scenario body returns `Err`, the scenario must
not be recorded as passed. Success is observable by compiling and running a
fallible scenario that uses `?`, by seeing a skipped fallible scenario return
`Ok(())`, and by confirming that a fallible scenario returning `Err` does not
leave a passed record in the reporting collector.

## Constraints

- Follow ADR-006 exactly: allow only `Result<(), E>` or `StepResult<(), E>`
  for fallible scenario bodies, return `Ok(())` on skips, and ensure `Err` does
  not record a pass.
- Reuse the existing return-type classifier in
  `crates/rstest-bdd-macros/src/return_classifier.rs`.
- Do not introduce new external dependencies.
- Keep module-level `//!` doc comments for any new module.
- No file may exceed 400 lines; split helpers if needed.
- Documentation updates must follow the project Markdown rules (80-column
  wrap, `make fmt`, `make markdownlint`, `make nixie`).
- Quality gates must pass before considering the change complete:
  `make check-fmt`, `make lint`, `make test`.

## Tolerances (exception triggers)

- Scope: if the change requires more than 10 files or 600 net lines,
  stop and escalate.
- Interface: if a public API signature must change in `rstest-bdd`, stop
  and escalate (new internal helpers are ok).
- Dependencies: if a new external crate is required, stop and escalate.
- Tests: if tests fail after 3 debugging attempts on the same issue,
  stop and escalate.
- Ambiguity: if the runtime behaviour for `Err` reporting is unclear or
  conflicts with ADR-006, stop and escalate with options.

## Risks

- Risk: `?` in fallible scenario bodies might bypass the error-handling
  shim if the body is not wrapped correctly. Severity: high Likelihood: medium
  Mitigation: wrap the body in a closure/async block so `?` is captured and
  matched before returning.

- Risk: async scenarios might need a different wrapper to avoid borrow
  or lifetime issues. Severity: medium Likelihood: medium Mitigation: use an
  `async { ... }` block and `await` the result so the error handling mirrors
  sync behaviour.

- Risk: skip handler return types could mismatch the scenario signature.
  Severity: medium Likelihood: low Mitigation: generate skip handler code based
  on the classified scenario return kind and add unit tests that assert the
  emitted tokens.

## Progress

- [x] (2026-01-30 00:00Z) Drafted ExecPlan for fallible scenario bodies.
- [x] (2026-01-31 00:00Z) Stage A: Confirmed scenario codegen and reporting.
- [x] (2026-01-31 00:00Z) Stage B: Added return classification and body wrapper.
- [x] (2026-01-31 00:00Z) Stage C: Updated skip handler for fallible returns.
- [x] (2026-01-31 00:00Z) Stage D: Added unit + behavioural tests.
- [x] (2026-01-31 00:00Z) Stage E: Updated docs and roadmap; ran quality gates.

## Surprises & discoveries

- Observation: `make fmt` relies on `fd`, which is not installed in this
  environment. Evidence: `mdformat-all` failed with
  `/root/.local/bin/mdformat-all: line 20: fd: command not found`. Impact:
  Introduced a temporary `fd` shim for formatting runs; quality gates still
  need to be re-run after final edits.

## Decision log

- Decision: Use the existing return classifier to detect scenario return
  kinds and reject `Result<T, E>` where `T != ()` with the ADR-006 diagnostic
  message. Rationale: Keeps step and scenario return semantics aligned while
  meeting the ADR requirements. Date/Author: 2026-01-30 / Codex

- Decision: Wrap fallible scenario bodies in a closure (sync) or async
  block (async) so `?` returns are captured and can be matched. Rationale:
  Prevents early returns from bypassing the scenario guard. Date/Author:
  2026-01-30 / Codex

- Decision: Generate skip handler code that returns `Ok(())` for fallible
  scenarios and `return;` for unit scenarios. Rationale: Ensures type-correct
  short-circuiting without affecting existing unit-return behaviour.
  Date/Author: 2026-01-30 / Codex

## Outcomes & retrospective

- Outcome: Added fallible scenario return support for `Result<(), E>` and
  `StepResult<(), E>`, returning `Ok(())` on skips and preventing `Err` results
  from recording a pass.
- Outcome: Added unit coverage for the skip handler and behavioural tests for
  success, skip, and error scenarios, plus a trybuild fixture for
  `Result<T, E>` rejection.
- Outcome: Updated the ergonomics design notes, user guide, and roadmap entry,
  then validated quality gates.
- Retrospective: `make fmt` still depends on `fd`; the temporary shim works but
  documenting or vendoring a fallback would reduce friction.

## Context and orientation

`#[scenario]` lives in `crates/rstest-bdd-macros/src/macros/scenario/mod.rs`
and generates tests via `crates/rstest-bdd-macros/src/codegen/scenario.rs`. The
runtime scaffolding (scenario guard, skip handler, step executor loop) comes
from `crates/rstest-bdd-macros/src/codegen/scenario/runtime/`.
`__RstestBddScenarioReportGuard` records `Passed` on drop if not already
recorded; this is the key mechanism that must be updated to avoid marking `Err`
returns as passed. The shared return-type classifier lives in
`crates/rstest-bdd-macros/src/return_classifier.rs` and already recognises
`Result` and `StepResult` paths.

Tests and fixtures to reference:

- `crates/rstest-bdd/tests/scenario.rs` for behavioural scenario coverage.
- `crates/rstest-bdd/tests/skip.rs` for skip handling and reporting.
- `crates/rstest-bdd/tests/trybuild_macros.rs` plus
  `crates/rstest-bdd/tests/ui_macros/` for compile-fail macro tests.
- `crates/rstest-bdd-macros/src/codegen/scenario/runtime/tests.rs` for
  generator unit tests.

Docs to update:

- `docs/users-guide.md` for the new usage and semantics.
- `docs/ergonomics-and-developer-experience.md` (design decisions) or the
  most relevant design section in `docs/rstest-bdd-design.md`.
- `docs/roadmap.md` to mark item 5.1.6 as done.

Reference documents for style and context (no direct changes expected):
`docs/rstest-bdd-design.md`, `docs/rstest-bdd-language-server-design.md`,
`docs/rust-testing-with-rstest-fixtures.md`, `docs/rust-doctest-dry-guide.md`,
`docs/complexity-antipatterns-and-refactoring-strategies.md`, and
`docs/gherkin-syntax.md`.

## Plan of work

Stage A: inspect and confirm current behaviour (no code changes). Read the
scenario macro, runtime generators, and reporting guard to identify the exact
injection points for return handling and skip returns. If the existing guard or
skip handler already handles fallible results, stop and update this plan.

Stage B: classify scenario return types and wrap fallible bodies. In
`crates/rstest-bdd-macros/src/macros/scenario/mod.rs`, classify the scenario
return type using `classify_return_type`. Introduce a small scenario-specific
return enum (for example, `ScenarioReturnKind`) with `Unit` and `ResultUnit`,
and reject `ResultValue` with the ADR-006 error message. Propagate the scenario
return kind into `ScenarioConfig` and `ScenarioMetadata` so runtime generation
can adapt. In `crates/rstest-bdd-macros/src/codegen/scenario/runtime.rs`, wrap
the scenario body in a closure or async block when `ResultUnit` so `?` and
`return Err(...)` are captured and matched. On `Err`, call
`__rstest_bdd_scenario_guard.mark_recorded()` before returning the error.

Stage C: update skip handler generation for fallible returns. In
`crates/rstest-bdd-macros/src/codegen/scenario/runtime/generators/ scenario.rs`,
 make `generate_skip_handler` accept the scenario return kind (or a boolean).
Emit `return Ok(())` for fallible scenarios and `return;` for unit scenarios.
Thread this choice through `generate_common_components` and the code component
assembly so both regular and outline scenarios get the correct skip handler.

Stage D: add tests.

Unit tests:

- Add generator tests in
  `crates/rstest-bdd-macros/src/codegen/scenario/runtime/tests.rs` to assert
  the skip handler emits `return Ok(())` for fallible scenarios and `return;`
  for unit scenarios.
- Add unit tests in a new or existing macro test module to verify that
  `Result<(), E>` is accepted and `Result<T, E>` (T != ()) yields the ADR-006
  compile-time error string.

Behavioural tests:

- Add a new feature file under `crates/rstest-bdd/tests/features/` with
  scenarios that exercise fallible bodies (one successful, one skipped).
- Add an integration test file under `crates/rstest-bdd/tests/` that
  defines `#[scenario]` functions returning `Result<(), E>` and uses `?` inside
  the body. Ensure the skipped scenario returns `Ok(())` and records a skipped
  status.
- Add a fallible scenario function that returns `Err` and mark it
  `#[ignore]`, then add a separate `#[test]` that calls it directly, asserts it
  returns `Err`, and confirms `drain_reports()` does not contain a `Passed`
  record for that scenario.

Trybuild tests:

- Add a new UI fixture under `crates/rstest-bdd/tests/ui_macros/` that
  defines a `#[scenario]` returning `Result<u8, E>` and asserts the compile
  error matches ADR-006. Register the fixture in
  `crates/rstest-bdd/tests/trybuild_macros.rs`.

Stage E: documentation and roadmap updates.

- Update `docs/users-guide.md` with a new subsection under the scenario
  or skipping guidance showing `Result<(), E>` and `StepResult<(), E>` bodies,
  plus a note that skipped scenarios return `Ok(())`.
- Update the relevant design document
  (`docs/ergonomics-and-developer-experience.md` or
  `docs/rstest-bdd-design.md`) to capture the final design and any decisions
  made during implementation.
- Mark roadmap item 5.1.6 as done in `docs/roadmap.md`.

Finish with formatting and quality gates, then re-check the plan for any new
decisions that must be logged.

## Concrete steps

All commands run from the repository root (`/home/user/project`). Use
`set -o pipefail` and `tee` for long outputs.

Stage A (read-only):

    rg -n "scenario" crates/rstest-bdd-macros/src/macros/scenario
    rg -n "generate_skip_handler" crates/rstest-bdd-macros/src/codegen/scenario

Stage B/C (after edits):

    cargo test -p rstest-bdd-macros

Stage D (after tests added):

    cargo test -p rstest-bdd --test scenario
    cargo test -p rstest-bdd --test skip
    cargo test -p rstest-bdd --test trybuild_macros

Stage E (docs + full validation):

    set -o pipefail && make fmt 2>&1 | tee /tmp/fmt.log
    set -o pipefail && make markdownlint 2>&1 | tee /tmp/markdownlint.log
    set -o pipefail && make nixie 2>&1 | tee /tmp/nixie.log
    set -o pipefail && make check-fmt 2>&1 | tee /tmp/check-fmt.log
    set -o pipefail && make lint 2>&1 | tee /tmp/lint.log
    set -o pipefail && make test 2>&1 | tee /tmp/test.log

## Validation and acceptance

The change is complete when:

- A `#[scenario]` function returning `Result<(), E>` can use `?` in its
  body and passes when it returns `Ok(())`.
- A skipped fallible scenario returns `Ok(())` and records a skipped
  status without executing the scenario body.
- A fallible scenario body that returns `Err` does not record a passed
  scenario (verified via `reporting::drain()` in a behavioural test).
- A scenario returning `Result<T, E>` with `T != ()` fails to compile
  with the ADR-006 message.
- `make check-fmt`, `make lint`, and `make test` all pass.

## Idempotence and recovery

All steps are re-runnable. If a stage fails, fix the cause and re-run the same
commands. For trybuild snapshots, update only the expected `.stderr` files
after validating the new diagnostics are correct.

## Artifacts and notes

Record any new diagnostics, helper function signatures, or test output snippets
here as they are produced during implementation.

## Interfaces and dependencies

Expected internal interfaces after this change (names may vary but the
semantics must exist):

In `crates/rstest-bdd-macros/src/macros/scenario/mod.rs`:

- A helper that classifies the scenario return type using
  `classify_return_type` and returns a `ScenarioReturnKind` (Unit or
  ResultUnit). It must reject `Result<T, E>` where `T != ()` with the ADR-006
  message.

In
`crates/rstest-bdd-macros/src/codegen/scenario/runtime/generators/scenario.rs`:

- `generate_skip_handler` should accept the scenario return kind and emit
  `return Ok(())` for fallible scenarios.

In `crates/rstest-bdd-macros/src/codegen/scenario/runtime.rs`:

- A helper that builds the scenario body tokens based on return kind and
  async mode, wrapping fallible bodies in a closure/async block and marking the
  scenario guard as recorded before returning `Err`.

## Revision note

- 2026-01-31: Updated progress through Stage D, documented the `fd` tooling
  gap during formatting, and noted remaining quality gates. This clarifies the
  remaining work is limited to running validation commands and addressing any
  resulting failures.
- 2026-01-31: Marked Stage E complete after running `make check-fmt`,
  `make lint`, `make test`, plus Markdown validation (`make markdownlint`,
  `make nixie`).
