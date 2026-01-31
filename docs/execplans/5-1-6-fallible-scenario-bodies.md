# ExecPlan 5.1.6: Fallible scenario bodies

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Plan of work`, `Concrete steps`, `Decision log`, and
`Outcomes & retrospective` must be kept up to date as work proceeds.

## Objective

Enable `#[scenario]` bodies to return `Result<(), E>` or `StepResult<(), E>`
while preserving existing skip behaviour and scenario reporting semantics.

## Context

File paths referenced below are current as of 2026-01-30 and may move. Prefer
module names and crate boundaries when locating code.

## Constraints

- Only unit `Result` payloads are allowed in scenario bodies.
- Skip handling must remain type-correct for fallible scenarios.
- Scenario guards must not record a pass when a fallible body returns `Err`.

## Plan of work

### Stage A: classify scenario return types

- Extend the scenario macro to classify return types using the shared return
  classifier.
- Accept only `()` and `Result<(), E>`/`StepResult<(), E>` for scenario bodies.
- Emit a compile-time error with a clear diagnostic for invalid return types.

### Stage B: wrap fallible scenario bodies

- For fallible scenarios, wrap the body in a closure (or async block) to capture
  `?` and `return Err(..)` results.
- On `Err`, mark the scenario guard as recorded to avoid `Passed` being
  recorded on drop, then return the error to the test harness.

### Stage C: update skip handler generation for fallible returns

- In the scenario runtime generator (currently
  `crates/rstest-bdd-macros/src/codegen/scenario/runtime/generators/scenario.rs`
   in `rstest-bdd-macros`), make `generate_skip_handler` accept the scenario
  return kind (or a boolean).
- Return `Ok(())` for fallible scenarios and `return;` for infallible ones.

## Validation

- Add trybuild fixtures rejecting non-unit `Result` and `StepResult` payloads.
- Add behavioural tests for successful and error fallible scenarios.
- Run `make check-fmt`, `make lint`, and `make test`.

## Concrete steps

1. Update scenario return classification in the `#[scenario]` macro (currently
   in `rstest-bdd-macros` under
   `crates/rstest-bdd-macros/src/macros/scenario/mod.rs`) to accept only unit
   return kinds.
2. Wrap fallible scenario bodies in the runtime scaffolding (currently in
   `rstest-bdd-macros` under `crates/rstest-bdd-macros/src/codegen/scenario`
   and `crates/rstest-bdd-macros/src/codegen/scenario/runtime`) to mark the
   guard before returning errors.
3. Adjust skip handler generation in the runtime generators (currently in
   `crates/rstest-bdd-macros/src/codegen/scenario/runtime/generators`) to
   return `Ok(())` for fallible scenarios.
4. Add trybuild fixtures and behavioural tests in the `rstest-bdd` test suite
   (currently under `crates/rstest-bdd/tests`) to cover success, error, and
   invalid return signatures.

## Decision log

- Decision: enforce unit-only payloads for fallible scenarios.
  Date/Author: 2026-01-30 / Codex.
- Decision: wrap fallible scenario bodies to mark guards on `Err`.
  Date/Author: 2026-01-30 / Codex.
- Decision: emit `Ok(())` from skip handlers for fallible scenarios.
  Date/Author: 2026-01-30 / Codex.

## Outcomes & retrospective

Capture final outcomes, follow-up tasks, and any refactors deferred during
delivery.
