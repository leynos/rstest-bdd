# Architectural decision record (ADR) 006: fallible scenario functions

## Status

Accepted (2026-01-28): Allow fallible scenario bodies returning `Result<(), E>`
or `StepResult<(), E>`, return `Ok(())` for skipped scenarios, and ensure `Err`
outcomes do not record a passed scenario.

## Date

2026-01-28.

## Context and problem statement

Scenario bodies currently return `()` while steps already support fallible
return types via the shared `ReturnKind` classification. See
[ADR 002](adr-002-stable-step-return-classification.md) for the return-kind
design and [Users guide §Step return values](users-guide.md#step-return-values)
for macro classification details. During the fallible scenario experiment, the
macro could not express a fallible scenario signature without breaking the skip
path or misreporting the scenario outcome. The framework needs a way to let
scenario bodies use `?` while keeping status recording accurate and compiler
diagnostics clear.

## Decision drivers

- Support fallible scenario bodies without additional boilerplate.
- Preserve accurate pass/fail reporting when a scenario returns `Err`.
- Keep the skip short-circuit path type-correct for fallible signatures.
- Align scenario handling with the existing step `ReturnKind` logic.
- Avoid introducing unused payload values in scenario results.

## Requirements

### Functional requirements

- Allow scenario bodies to return `Result<(), E>` or `StepResult<(), E>`.
- When a scenario is skipped, return `Ok(())` to satisfy the fallible
  signature.
- When a scenario returns `Err`, mark the scenario as recorded so it is not
  reported as passed and propagate the error to the test harness.

### Technical requirements

- Reuse the existing return-type classification logic used by steps.
- Provide a clear compile-time error for `Result<T, E>` where `T != ()`.

## Options considered

### Option A: Keep scenario bodies infallible

Continue to require `fn scenario() -> ()`, and force fallible work into steps
or helper functions. This avoids API changes but keeps fallible scenario bodies
out of reach and complicates scenarios that only need a small fallible setup.

### Option B: Allow any `Result<T, E>` and discard payloads

Accept `Result<T, E>` for scenario bodies and ignore `T`. This introduces a
footgun: the payload is silently discarded, and it is not obvious where to use
it, making the API misleading.

### Option C: Allow only `Result<(), E>` or `StepResult<(), E>` (selected)

Extend the macro to accept fallible unit results only. The skip path returns
`Ok(())`, and the runtime wrapper marks the scenario as recorded before
returning an `Err`, ensuring the test fails without being reported as passed.

| Topic                     | Option A | Option B  | Option C |
| ------------------------- | -------- | --------- | -------- |
| Fallible ergonomics       | Poor     | Mixed     | Good     |
| Type clarity              | High     | Low       | High     |
| Result payload semantics  | N/A      | Confusing | Clear    |
| Implementation complexity | Low      | Medium    | Medium   |

_Table 1: Trade-offs between the options._

## Decision outcome / proposed direction

Adopt Option C. The `#[scenario]` macro now classifies scenario return types
using the same `ReturnKind` logic as steps and permits `Result<(), E>` and
`StepResult<(), E>` only. The skip handler returns `Ok(())` for fallible
scenarios, and the fallible body wrapper marks the scenario as recorded before
propagating `Err` to the test harness. Returning `Result<T, E>` where `T != ()`
produces a compile-time error: "`#[scenario]` bodies must return () or a unit
Result/StepResult".

For screen readers: The following snippet shows a fallible scenario body using
`Result<(), E>`.

```rust,no_run
#[scenario(path = "tests/features/example.feature")]
fn my_scenario() -> Result<(), MyError> {
    do_something_fallible()?;
    Ok(())
}
```

## Goals and non-goals

### Goals

- Enable scenario bodies to be fallible without extra wrappers.
- Keep skip handling type-correct for fallible scenarios.
- Ensure `Err` results fail the test without being recorded as passed.

### Non-goals

- Support `Result<T, E>` payloads from scenario bodies.
- Change step fallibility semantics or step return handling.
- Introduce additional scenario outcome variants beyond existing recording.

## Architectural rationale

- Reuses existing return-type classification to keep macro behaviour
  consistent across steps and scenarios.
- Preserves the recorder drop-guard semantics by explicitly marking an `Err`
  scenario as recorded before returning.
- Keeps API surface small while enabling a common fallible use case.

## Known risks and limitations

- Scenarios that previously returned `Result<T, E>` with a payload will now
  fail to compile and must move payloads into fixtures or state.
- The error message is fixed to unit-return expectations; any future need for
  payloads would require a new ADR and API design.
