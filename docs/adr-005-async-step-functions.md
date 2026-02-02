# Architectural decision record (ADR) 005: async step functions

## Status

Superseded (2026-01-30): `rstest-bdd` now supports native `async fn` step
definitions under `tokio-current-thread` async scenarios, eliminating the
sync-wrapper execution path that prompted this ADR. The guidance below remains
useful as an interim migration strategy for older releases and for codebases
that prefer to keep steps synchronous.

## Date

2026-01-24.

## Context and Problem Statement

While migrating `wireframe` from Cucumber to `rstest-bdd`, async operations
(network I/O, codec work, and StreamEnd helpers) need to run inside step
functions. The current implementation pattern keeps step functions synchronous
and, when necessary, uses a per-step Tokio runtime to `block_on` async calls.

The `scenarios!` macro already supports running scenarios with a Tokio runtime
(e.g. `tokio-current-thread`). Moving to fully async step functions would
require a broader step rewrite and risks nested runtime failures. The core
question is whether async step functions should be introduced now, or whether
the current synchronous step model with targeted async bridging should be
retained.

## Decision Drivers

- Preserve step ergonomics and minimize migration rewrites from Cucumber.
- Avoid runtime-in-runtime failures and other Tokio constraints.
- Keep scenario execution deterministic and debuggable.
- Allow async I/O where it is essential for the migration.
- Keep options open for future async step support in `rstest-bdd`.

## Requirements

### Functional requirements

- Steps must be able to invoke async operations needed for the migration.
- Scenario execution must remain stable with no hidden runtime nesting.

### Technical requirements

- Works with the existing `scenarios!` macro runtime configuration.
- Avoids blocking the runtime in ways that violate Tokio constraints.

## Options Considered

### Option A: Synchronous scenarios and synchronous steps

Run scenarios without an async runtime. Steps remain synchronous and construct
a per-step runtime when an async call is unavoidable.

### Option B: Async scenarios with synchronous steps (proposed)

Run scenarios on `tokio-current-thread` via the `scenarios!` macro. Keep steps
synchronous, and bridge async operations with per-step runtimes only when the
step cannot be refactored into async fixtures or helpers.

### Option C: Async scenarios with async step functions

Rewrite steps as `async fn` so each step executes within a shared scenario
runtime. This avoids per-step runtimes, but requires extending `rstest-bdd`
step support and rewriting existing steps.

| Topic                  | Option A | Option B | Option C |
| ---------------------- | -------- | -------- | -------- |
| Migration effort       | Low      | Low      | High     |
| Runtime nesting risk   | Medium   | Medium   | Low      |
| Step ergonomics        | Stable   | Stable   | Mixed    |
| Framework changes      | None     | None     | Required |
| Async support fidelity | Limited  | Limited  | High     |

_Table 1: Trade-offs between the options._

## Decision Outcome / Proposed Direction

Adopt Option B. Keep step functions synchronous to avoid widespread rewrites,
run scenarios with `tokio-current-thread` to support async fixtures, and use
per-step runtimes only when a step cannot be refactored into async fixture
helpers. This gives the migration a stable path while preserving a future
upgrade path to true async steps once `rstest-bdd` supports them directly.

### Update (2026-01-30)

Native async step execution is now implemented. Step functions may be declared
as `async fn` and are awaited sequentially under async scenario runtimes. Async
steps can still run in synchronous scenarios via a blocking fallback, but that
fallback refuses to create a nested Tokio runtime when one is already running.

## Goals and Non-Goals

### Goals

- Support StreamEnd and CodecStateful migrations without reauthoring all steps.
- Document the runtime strategy so it is applied consistently across features.

### Non-Goals

- Provide first-class async step functions in `rstest-bdd` as part of the
  initial migration strategy described in this ADR (superseded as of
  2026-01-30).
- Guarantee zero runtime overhead for async operations within steps.

## Migration Plan

### Phase 1: Fixture-first synchronous steps

- Keep step functions synchronous and move async work into fixtures.
- Use `tokio-current-thread` for async scenario execution.

### Phase 2: Prototype async step execution

- Prototype async step execution behind a feature flag.

### Phase 3: Stabilize async step API

- Stabilize the async step API and publish migration notes.

## Architectural Rationale

- Aligns with the single-threaded runtime and `RefCell`-backed fixture model.
- Preserves deterministic step ordering and avoids nested runtimes.
- Maintains step ergonomics while enabling future async support.

## Known Risks and Limitations

- Per-step runtimes add overhead and complicate tracing of async failures.
- Runtime nesting can still occur if async helpers are called from inside an
  already-running runtime.
- Some async APIs may require refactoring into fixtures to avoid blocking.

## Outstanding Decisions

- Confirm whether any steps must be fully async rather than moved into fixtures.
- Decide the minimal API changes needed to support async steps in the future.
- Record the migration pattern in the execplan and `known-issues` once the
  StreamEnd and CodecStateful migrations are complete.
