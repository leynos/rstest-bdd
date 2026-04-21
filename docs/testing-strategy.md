# Testing strategy

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

These assertions are more resilient to refactors in the generated step loop
because they validate the contract users observe rather than the exact tokens
that happen to implement it.

## Invariants to prefer

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

## Recommended patterns

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

## Good and fragile assertions

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

## Test support infrastructure

Async semantic behaviour tests share a support module at
`tests/common/async_semantic_behaviour_support.rs`. The types and helpers below
should be used instead of raw strings wherever assertions require structured
context.

### Parameter structs

| Type                                                 | Purpose                                                                                                          |
| ---------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `ScenarioRef<'a>`                                    | Bundles the scenario `name` and `feature_suffix` used in failure-context assertions.                             |
| `StepRef<'a>`                                        | Bundles the step `keyword`, `text`, `function_name`, and `handler_error` for failure-context assertions.         |
| `BypassedStepQuery<'a>` _(diagnostics feature only)_ | Bundles `scenario_name`, `scenario_line`, `step_pattern`, and `reason` for bypassed-step diagnostics assertions. |

Prefer struct-literal syntax at call sites so that each field is labelled and
the intent is clear:

```rust
assert_handler_failure_context(
    &message,
    ScenarioRef { name: ERROR_SCENARIO_NAME, feature_suffix: FEATURE_PATH },
    StepRef {
        keyword:       "When",
        text:          "a step fails with an error",
        function_name: "step_that_fails",
        handler_error: "deliberate failure",
    },
);
```

### Assertion helpers

| Function                                                                        | What it checks                                                                                                                                                    |
| ------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `assert_feature_path_suffix(actual, expected_suffix)`                           | Verifies that a file path ends with the expected suffix using `Path::ends_with`.                                                                                  |
| `assert_handler_failure_context(message, ScenarioRef, StepRef)`                 | Normalises a panic message and asserts it matches a regex covering step keyword, step text, function name, handler error, feature path suffix, and scenario name. |
| `assert_bypassed_step_recorded(BypassedStepQuery)` _(diagnostics feature only)_ | Parses the diagnostics registry JSON and asserts a matching bypassed-step entry exists.                                                                           |

### Event and cleanup utilities

| Function                              | Purpose                                                                                                           |
| ------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `clear_events()`                      | Resets the per-thread event log; call at the start of every test that reads from it.                              |
| `push_event(event)`                   | Appends a string to the per-thread event log from within a step handler.                                          |
| `snapshot_events() -> Vec<String>`    | Returns a snapshot of the current event log without clearing it.                                                  |
| `reset_cleanup_drops()`               | Resets the per-thread drop counter; call before the scenario under test.                                          |
| `cleanup_drops() -> usize`            | Returns the number of times `CleanupProbe` has been dropped in this thread.                                       |
| `scenario_line(scenario_name) -> u32` | Reads the feature file and returns the 1-based line number of the named scenario; avoids hard-coded line numbers. |

### Thread-local state

All mutable state (`events`, `cleanup_drops`) is held in a single
`thread_local! { TestState }`. Isolation is therefore per-thread; any test that
reads from shared state must call the corresponding reset helper before running
its scenario. Tests that mutate shared state must be annotated with `#[serial]`
to prevent interleaving with other tests on the same thread pool.
