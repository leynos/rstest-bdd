# Architectural decision record (ADR) 001: async fixtures and tests (Tokio)

## Status

Proposed.

## Date

2025-12-13.

## Context

`rstest-bdd` currently executes step definitions synchronously. The runtime
step registry stores a function pointer (`StepFn`) which is invoked immediately
and returns a `Result`. The scenario runner executes the resolved steps
sequentially by calling that function pointer.

This design prevents Tokio-style asynchronous step definitions from being
executed correctly:

- An `async fn` step definition produces a `Future`.
- The current wrapper code calls the function and treats the `Future` as an
  ordinary return value, so the work is never awaited.

The crate already supports `rstest` fixtures being injected into the scenario
test function. However, to support Tokio asynchronous fixtures and steps in a
way that matches user expectations, the step execution boundary must become
asynchronous end-to-end.

The most significant constraint is the mutable fixture model. `rstest-bdd`
stores owned fixtures in `RefCell` containers, so steps can borrow mutable
references. In Rust async code, holding non-`Send` borrows or guards across
`.await` points constrains which Tokio runtime configuration can execute the
steps safely.

## Coexistence and migration

Async support must coexist with the current synchronous implementation.
Consumers should not be forced onto Tokio merely by upgrading `rstest-bdd`.

The migration strategy needs to define:

- How async support is enabled (feature flag, macro argument, or a dedicated
  attribute).
- What the default remains for existing projects (expected to remain
  synchronous).
- Whether synchronous scenarios remain supported without Tokio (expected to
  remain supported indefinitely).

One plausible approach is a dual execution pipeline:

- Synchronous pipeline remains the default for step definitions and scenario
  generation.
- Async pipeline is opt-in and only activated when a scenario test (or a
  `scenarios!` invocation) explicitly selects Tokio.

This preserves the current behaviour for users who do not need async support,
whilst allowing async projects to adopt Tokio step execution incrementally.

## Decision drivers

- Step definitions should support `async fn` under Tokio.
- `rstest` asynchronous fixtures should remain usable in scenario tests.
- Skipping (`skip!`) and error reporting must continue to produce actionable
  diagnostics.
- The default path should minimize disruption to existing step signatures and
  fixture usage.
- The design should keep the cost model predictable (allocations, dynamic
  dispatch, and runtime constraints).

## Requirements

### Functional requirements

- Step definitions must support both `fn` and `async fn` forms.
- Scenario execution must await each step sequentially (no concurrent step
  scheduling).
- Scenario tests must support `rstest` asynchronous fixtures (including cases
  which require `#[future]` bindings).
- The `scenarios!` macro must be able to generate Tokio-compatible tests
  because generated test functions cannot be manually annotated with
  `#[tokio::test]`.
- `skip!` must continue to stop scenario execution and record a skipped
  outcome with an optional message.
- Failures must continue to identify the step index, keyword, and text, and
  include feature/scenario metadata.

### Technical requirements

- The step registry must store step wrappers which can be awaited.
- The step registry must store an async step wrapper whose signature ties the
  returned future to the lifetime of the borrowed `StepContext` and therefore
  cannot be a `'static` future.

  For screen readers: The following Rust snippet outlines a likely stored step
  wrapper signature for async execution.

  ```rust,no_run
  use std::future::Future;
  use std::pin::Pin;

  type StepFuture<'a> =
      Pin<Box<dyn Future<Output = Result<StepExecution, StepError>> + 'a>>;

  type StepFn = for<'a> fn(
      &'a mut StepContext<'a>,
      &str,
      Option<&str>,
      Option<&[&[&str]]>,
  ) -> StepFuture<'a>;
  ```

  This shape constrains unwind handling (the implementation needs
  `Future`-aware unwind capture to preserve `skip!` interception) and the
  required trait bounds (`Send` depends on current-thread versus multi-thread
  mode).
- Wrapper generation must normalize sync and async step definitions into a
  single callable interface.
- The implementation must preserve unwind handling, so panics continue to be
  surfaced with context, and `skip!` continues to be intercepted.
- The approach must define and document the Tokio runtime constraints for
  step futures (for example, whether `Send` is required).

## Options considered

### Option A: Tokio current-thread mode

In this mode, scenario tests run on a Tokio current-thread runtime
(`#[tokio::test(flavor = "current_thread")]`). Step wrappers may return
non-`Send` futures.

This option aligns with the existing `RefCell`-backed mutable fixture design.
It permits step futures to hold `RefMut` guards or `&mut T` borrows across
`.await` points without requiring additional synchronization primitives.

Consequences:

- Minimal changes to fixture storage are required.
- Step definitions can use `&mut T` fixtures and await within the step.
- A current-thread runtime reduces incidental concurrency and can make
  starvation bugs easier to reproduce, but it may underutilize multicore
  systems for tests that spawn background tasks.
- Crates and test code that rely on spawning `Send` tasks onto the Tokio
  multi-thread scheduler may require additional configuration or refactoring.

### Option B: Tokio multi-thread (Send) mode

In this mode, scenario tests run on a Tokio multi-thread runtime
(`#[tokio::test(flavor = "multi_thread")]`). Step wrappers must return `Send`
futures, and any values captured across `.await` points must be `Send`.

This option improves compatibility with Tokio patterns that assume a
multi-thread runtime and `Send` tasks. It also enables more test-local
parallelism when step implementations spawn background work.

The primary cost is a redesign of the mutable fixture model. `RefCell` guards
and borrowed references are typically non-`Send` across `.await` points, so
steps which await while holding `&mut T` would not be supported without a new
indirection (for example, wrapping owned fixtures in `tokio::sync::Mutex` and
passing a guard-backed `&mut T`).

Consequences:

- More invasive changes to `StepContext` and fixture binding are required.
- Step signatures may need to change, or step wrappers must introduce mutex
  guards which are held across `.await`.
- Additional synchronization may reduce performance for heavily mutable
  scenarios and introduces deadlock risks if locks are held too broadly.
- The design must define clear guidance for preventing long-lived locks across
  `.await` points.

Table 1 compares the two Tokio modes.

| Topic                             | Current-thread                    | Multi-thread (Send)                      |
| --------------------------------- | --------------------------------- | ---------------------------------------- |
| Step futures                      | `!Send` permitted                 | `Send` required                          |
| Mutable fixtures across `.await`  | Works with `RefCell` and `&mut T` | Requires redesign (for example, mutexes) |
| Compatibility with `tokio::spawn` | Limited to `spawn_local` patterns | Compatible with `tokio::spawn`           |
| Implementation complexity         | Lower                             | Higher                                   |
| Risk surface                      | Lower                             | Higher (locks, `Send` constraints)       |

_Table 1: Trade-offs between Tokio current-thread and multi-thread modes._

## Tokio wrapping for generated tests

Manual scenario tests already permit the user to choose the runtime by
annotating the test function (for example, `#[tokio::test]`) and writing an
`async fn`. Auto-generated tests from `scenarios!` cannot be annotated by the
user, so the macro must emit Tokio-compatible tests when Tokio is selected.

One design is to add a macro argument selecting the runtime. For example:

```rust,no_run
rstest_bdd::scenarios!("tests/features", runtime = "tokio-current-thread");
```

The expansion would generate `async fn` tests and attach the Tokio test
attribute. The following is illustrative, not an exact expansion:

```rust,no_run
mod features_scenarios {
    use super::*;

    #[rstest::rstest]
    #[tokio::test(flavor = "current_thread")]
    async fn login_happy_path() {
        // Build StepContext, then execute and await each step wrapper.
    }
}
```

Alternative designs include:

- A separate macro (for example, `tokio_scenarios!`) which always generates
  Tokio-backed tests.
- A helper attribute macro which wraps an existing `#[rstest::rstest]` test in
  a Tokio runtime.

## Current-thread limitations and failure modes

When Tokio current-thread is the initial runtime, expected limitations and
failure modes should be documented, so users can select an appropriate mode.

- Blocking operations (for example, `std::thread::sleep`, blocking I/O, or CPU
  heavy work) will block the entire runtime thread and can stall the scenario.
  Users may need to move blocking work to `tokio::task::spawn_blocking` or
  refactor the step to use async I/O.
- Code which assumes a multi-thread runtime and uses `tokio::spawn` may fail
  when the future captured by the spawned task is `!Send`. In that case, users
  must switch to `spawn_local` patterns (and potentially `LocalSet`), or select
  multi-thread mode.
- Nested runtimes can fail at runtime. For example, a test already running
  under `#[tokio::test]` should not attempt to create and block on a new Tokio
  runtime within a step.
- When step definitions hold mutable borrows across `.await`, they are
  inherently coupled to a single-threaded execution model. This can complicate
  later migration to multi-thread mode and should be treated as a design
  constraint rather than an incidental implementation detail.

## Outstanding decisions

- Which Tokio mode should be the default for generated tests:
  current-thread, multi-thread, or a user-selected configuration.
- How Tokio selection should be expressed:
  - feature flags (for example, `rstest-bdd-macros/tokio`),
  - macro arguments (for example, `scenarios!(…, runtime = "tokio")`), or
  - a dedicated attribute macro which expands to the appropriate test wrapper.
- Whether `rstest-bdd` should support both modes simultaneously, and if so,
  whether that is a compile-time selection or a per-test selection.
- The target async interface for the step registry:
  - function pointers returning boxed futures, or
  - a trait object abstraction around step invocation.
- The unwind and skip handling strategy for async steps, including the choice
  of dependency for `Future`-aware `catch_unwind` support.
- The required trait bounds for stored step functions and step futures
  (`Send`, `Sync`, `'static`, and unwind-safety requirements).
- The storage model for owned fixtures and step-return overrides under
  multi-thread mode (for example, `tokio::sync::Mutex`, `RwLock`, or a
  different borrowing API).
- The policy for mixed sync and async step definitions within a single
  scenario, including error messaging when an unsupported form is used.
- The intended scope beyond Tokio (for example, parity for `async-std`) and
  whether the runtime selection model should be generalized.

## Proposed direction

Implement Tokio current-thread mode first to unlock correct async step and
fixture behaviour with minimal disruption, then evaluate multi-thread support
as a follow-on ADR once the fixture storage model and `Send` requirements are
clear.

## Related decisions

ADR-004 introduces a shared `rstest-bdd-policy` crate to centralize
`RuntimeMode` and `TestAttributeHint` for both the runtime and macro crates.
This ADR does not change the async execution direction above; it only removes
policy duplication between crates.
