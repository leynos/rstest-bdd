# Architectural decision record (ADR) 007: harness context injection mechanism

## Status

Accepted (2026-03-03): Adopt an associated `Context` type on `HarnessAdapter`
and thread it through `ScenarioRunRequest` and `ScenarioRunner`.

## Date

2026-03-03.

## Context and problem statement

`HarnessAdapter::run` currently receives `ScenarioRunRequest<'_, T>` where the
runner is `FnOnce() -> T`. That closure is opaque to the harness. A harness can
observe metadata, but it cannot provide framework-owned resources to scenario
execution in a typed way.

This blocks framework integrations that need to supply runtime resources at the
harness boundary, such as GPUI's `TestAppContext` or Bevy's `bevy::ecs::World`.

## Decision drivers

- Enable typed context handoff from harness to scenario runner.
- Keep framework-specific crates opt-in (ADR-005).
- Avoid hidden global state and improve deterministic behaviour.
- Keep migration cost manageable for existing `StdHarness` and `TokioHarness`.
- Preserve a small, stable core trait surface for third-party harnesses.

## Options considered

### Option A: thread-local convention

Use a thread-local slot for harness context. Harnesses would set TLS before
calling `request.run()` and clear it after.

Pros:

- Minimal API changes.
- Works with existing closure signatures.

Cons:

- Hidden coupling and implicit global state.
- Fragile in nested harness calls and harder to reason about in tests.
- Poor fit for deterministic, explicit fixture injection design.

### Option B: associated `Context` type on `HarnessAdapter` (selected)

Add an associated `Context` type and make runner/request generic over context:

```rust,no_run
pub trait HarnessAdapter {
    type Context;

    fn run<T>(&self, request: ScenarioRunRequest<'_, Self::Context, T>) -> T;
}
```

Pros:

- Explicit, typed contract between harness and runner.
- No thread-local state.
- Fits GPUI and Bevy equally: each harness chooses its own context type.
- Keeps framework dependencies out of core crates.

Cons:

- Breaking trait API change.
- Requires migration across harness implementations and macro code generation.

### Option C: `StepContext` extension trait

Define an extension trait over runtime `StepContext` for harness injection.

Pros:

- Injection-oriented API at the runtime boundary.

Cons:

- Couples harness abstraction to runtime storage internals.
- Complicates crate boundaries introduced by ADR-005.
- Reduces flexibility for harnesses that do not map 1:1 to `StepContext`
  semantics.

| Topic                     | Option A      | Option B | Option C       |
| ------------------------- | ------------- | -------- | -------------- |
| Type safety               | Low           | High     | Medium         |
| Hidden global state       | High          | None     | None           |
| ADR-005 layering fit      | Medium        | High     | Low            |
| GPUI + Bevy portability   | Medium        | High     | Medium         |
| Migration complexity      | Low           | Medium   | Medium to high |
| Long-term maintainability | Low to medium | High     | Medium         |

_Table 1: Trade-offs between fixture-injection approaches._

## Decision outcome / proposed direction

Adopt Option B.

The harness contract is now:

- `HarnessAdapter` defines `type Context`.
- `ScenarioRunner<'a, C, T>` wraps `FnOnce(C) -> T`.
- `ScenarioRunRequest<'a, C, T>` threads `C` through request execution.
- `StdHarness` and `TokioHarness` use `Context = ()` and call
  `request.run(())`.

Macro-generated harness delegation now builds a runner closure that accepts the
harness context type:

```rust,no_run
# use rstest_bdd_harness::{HarnessAdapter, ScenarioRunRequest, ScenarioRunner};
# fn demo<H: HarnessAdapter>(request: ScenarioRunRequest<'_, H::Context, ()>) {
let _runner = ScenarioRunner::new(
    move |_harness_context: <H as HarnessAdapter>::Context| {
        // scenario runtime body
    },
);
# let _ = request;
# }
```

This establishes the typed handoff point required by GPUI and Bevy adapters,
while keeping framework-specific conventions in opt-in harness crates.

## Migration plan

1. Phase 1: core trait and runtime threading.
   Goal: establish an explicit typed context contract at the harness boundary.
   Deliverables:
   - Add `type Context` to `HarnessAdapter`.
   - Thread context generics through `ScenarioRunner` and
     `ScenarioRunRequest`.
   - Update core harness tests to cover context propagation and metadata
     invariants.

2. Phase 2: harness and macro migration.
   Goal: migrate built-in adapters and generated code to the new contract.
   Deliverables:
   - Update `StdHarness` and `TokioHarness` to set `Context = ()` and call
     unit-context helpers.
   - Update macro code generation to emit runner closures that accept the
     harness context argument.
   - Validate trybuild coverage for generated harness wrappers.

3. Phase 3: downstream adoption and documentation.
   Goal: make migration requirements explicit for third-party harness authors.
   Deliverables:
   - Document migration guidance for custom harness crates (`type Context`,
     runner constructor, and request execution updates).
   - Record the decision and rollout details in roadmap and user-facing docs.

## Goals and non-goals

### Goals

- Provide an explicit, typed fixture-injection mechanism at the harness
  boundary.
- Keep the core harness crate framework-agnostic and dependency-light.
- Support both GPUI and Bevy adapter designs.

### Non-goals

- Define a universal fixture-name mapping convention in this ADR.
- Implement GPUI or Bevy adapters in this phase.
- Replace `StepContext` internals.

## Known risks and limitations

- This is a source-breaking API for custom harnesses.
- Harness crates still need framework-specific conventions for mapping context
  values to step-facing fixtures.

## Architectural rationale

Associated context keeps the contract explicit and local, avoids global state,
and preserves ADR-005's crate boundary strategy. It provides a minimal core API
that can host multiple framework integrations without hard-coding framework
semantics into the runtime or macro crates.
