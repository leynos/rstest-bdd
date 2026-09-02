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


### LSP smoke-test synchronization

The language-server smoke tests exercise an asynchronous JSON-RPC process, so a
response can be interleaved with indexing notifications. A definition request
is valid only after the workspace index contains both the feature step and its
Rust implementation. Maintain this sequence in definition-navigation tests:

1. Complete the `initialize` handshake.
2. Save both the feature file and the Rust step file using `index_and_wait`, or
   an equivalent deterministic helper.
3. Wait for `textDocument/publishDiagnostics` for the Rust file URI. This is
   the completion boundary: the ordered saves have been processed, and the
   feature index and step registry are ready for navigation.
4. Send `textDocument/definition` and receive its response by JSON-RPC id, so
   buffered notifications cannot be mistaken for the response.

The completion boundary is a protocol invariant, not a timing hint. Do not use
sleeps, assumed scheduling delays, or platform-specific branches to make these
tests pass. Setting `--debounce-ms 0` can remove an intentional debounce, but
cannot replace the indexing-completion wait.

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
`crates/rstest-bdd/tests/common/async_semantic_behaviour_support.rs`. The types
and helpers below should be used instead of raw strings wherever assertions
require structured context.

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
| `assert_handler_failure_context(message, ScenarioRef, StepRef)`                 | Normalizes a panic message and asserts it matches a regex covering step keyword, step text, function name, handler error, feature path suffix, and scenario name. |
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
`thread_local! { static TEST_STATE: RefCell<TestState> = ... }` binding.
Isolation is therefore per-thread; any test that reads from shared state must
call the corresponding reset helper before running its scenario. Tests that
mutate shared state must be annotated with `#[serial]` to prevent interleaving
with other tests on the same thread pool.

## Cargo-spawning fixture-crate tests

Roadmap 10.3.3 introduced a distinct class of test: a fixture crate is copied
into the shared workspace `target/`, its manifest's relative path dependencies
are rewritten to absolute paths, and a nested `cargo` invocation is driven
against it with a controlled child environment. The following guarantees apply:

- The checked-in fixture is never mutated; all edits happen in the scratch
  copy, so a killed test is recovered by deleting the scratch directory.
- The child environment differs from the parent's only where cross-talk or
  stalls would result (`CARGO_MAKEFLAGS`, `CARGO_PKG_*`, and `CARGO_LLVM_COV*`
  stripped; `LLVM_PROFILE_FILE` redirected; `CARGO_TARGET_DIR` inherited or
  defaulted to the workspace target).
- Every nested `cargo` invocation runs under the harness's own wall-clock
  bound, and the binary is serialized against other cargo-spawning tests
  through the `cargo-spawning` nextest test-group.
- The regression suite's dep-info assertion is a direct filesystem check
  (rustc's `.d` file), deliberately _not_ mediated by the macros under test, so
  a macro bug cannot mask the regression it is meant to prove.

See `crates/rstest-bdd/tests/feature_rebuild_invalidation/` for the worked
harness and `docs/developers-guide.md` for the conventions.

## Tested living documentation

A second new class of test executes fenced examples extracted from user-facing
Markdown (roadmap 10.3.3, Milestone 7, modelled on
[`netsuke`](https://github.com/leynos/netsuke)). Markers
(`<!-- tested-example: id -->`) key each executable example; enforcement is
regional, so the documentation cannot quietly acquire an untested example
inside an enforced section. The recipe in the users-guide's rebuild
invalidation section is the first such example and is executed end-to-end: it
is written into a fixture crate's `build.rs` and a behavioural test proves a
newly added `.feature` file is run. The rule of thumb: prose that must not rot
when executed — recipes, configs, commands — should be marked and consumed by a
test rather than duplicated.

## Assertion posture

This repository adopted `googletest` and `pretty_assertions` in roadmap 10.3.3
(ExecPlan Decision D1, previously unused anywhere in the workspace). Use
`assert_that!` / `expect_that!` and matchers where the assertion expresses a
property (`contains_substring`, `eq`, `is_true`, `len`, `each`), and
`pretty_assertions` where a structural-equality diff carries the value.
`expect_that!` (which aggregates several failures into one report) requires the
`#[gtest]` attribute; inside `#[scenario]`-generated bodies there is no
`#[gtest]` context, so step functions use the panic-mode `assert_that!`.
