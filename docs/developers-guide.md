# Developer guide

## Internal test infrastructure

The async semantic behaviour tests use a shared support module at
`crates/rstest-bdd/tests/common/async_semantic_behaviour_support.rs`. Use the
helpers and types below when writing or extending semantic tests; do not access
`TEST_STATE` directly.

### Constants

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

Fields: `scenario_name: &'a str`, `scenario_line: u32`,
`step_pattern: &'a str`, `reason: &'a str`.

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

| Function                           | Purpose                                                                           |
| ---------------------------------- | --------------------------------------------------------------------------------- |
| `clear_events()`                   | Resets the per-thread event log. Call at the start of any test that reads events. |
| `push_event(event)`                | Appends a string to the per-thread event log. Call from within step handlers.     |
| `snapshot_events() -> Vec<String>` | Returns a clone of the current event log without clearing it.                     |

### Cleanup utilities

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

Use the same pattern in hand-written tests instead of bare `.unwrap()`. This
keeps the concrete `HarnessError` visible in the panic message when a harness
cannot initialize its infrastructure.

### HarnessError extension

`HarnessError` is marked `#[non_exhaustive]`, so downstream code that matches
on it must include a `_` fallback arm. New variants may be added in minor
releases as more harness infrastructure failures become typed and inspectable.
