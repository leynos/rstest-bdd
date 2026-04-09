# Testing Strategy

This project uses both structural macro tests and semantic behaviour tests.
They serve different purposes and should not be substituted for one another.

Structural tests are appropriate when validating code generation details such
as compile errors, emitted attributes, or token-level transformations. They are
useful for macro internals, but they are intentionally close to the current
implementation shape.

Semantic behaviour tests assert runtime-observable outcomes instead:

- whether steps stop after a skip or failure
- whether step ordering matches the feature declaration order
- whether fixtures remain available across step boundaries
- whether panic and error messages preserve scenario context
- whether cleanup still happens when execution exits early

These assertions are more resilient to refactors in the generated step loop,
because they validate the contract users observe rather than the exact tokens
that happen to implement it.

## Invariants to Prefer

When adding scenario execution coverage, prefer tests that enforce invariants
like these:

- Skip propagation: a skipped step must halt later steps, preserve its message,
  and record any bypassed steps in diagnostics output.
- Step ordering: background, Given, When, Then, and outline/example execution
  must preserve declaration order.
- Error propagation: handler failures should surface feature path, scenario
  name, step index, and step context in the final panic message.
- Fixture lifecycle: mutable fixtures should survive cross-step borrows, values
  returned from one step should be available to later steps, and owned fixtures
  should still drop when a scenario fails.

## Recommended Patterns

- Use real feature files plus `#[scenario]` or `scenarios!` so the runtime path
  matches production behaviour.
- Prefer event logs, counters, and final fixture assertions over token-stream
  inspection.
- For panic assertions, wrap scenario execution with `catch_unwind` or inspect
  Tokio `JoinError` panics, then assert on the rendered message.
- For skip assertions, inspect reporter output and, when diagnostics are
  enabled, assert against bypassed-step metadata from `dump_registry()`.
- For cleanup assertions, use lightweight RAII probes with `Drop` side effects
  rather than internal implementation hooks.

## Good and Fragile Assertions

Good semantic assertions:

- "the trailing step did not run after `skip!()`"
- "the panic includes the failing step text and scenario name"
- "the `RefCell` fixture still contains the expected value in the scenario body"

Fragile structural assertions:

- "the generated loop contains a particular helper name"
- "the macro emitted tokens in a specific statement order"
- "the expansion uses a particular temporary variable layout"

The review discussion that led to issue `#395` is a concrete reminder that
documentation and runtime behaviour can drift independently of code shape.
Semantic tests are the backstop that keeps those contracts aligned.
