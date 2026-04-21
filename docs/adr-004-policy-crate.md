# Architectural decision record (ADR) 004: shared policy crate

## Status

Accepted (2026-04-12)

## Date

2026-01-17

## Context and problem statement

The original runtime crate (`rstest-bdd`) defined `RuntimeMode` and
`TestAttributeHint` in `rstest_bdd::execution`. The proc-macro crate
(`rstest-bdd-macros`) could not depend on the runtime crate, so it maintained a
parallel copy of those enums for compile-time use. The duplication created a
manual synchronization burden and invited drift between the macro and runtime
layers.

The workspace needs a single source of truth for the runtime policy types that
both crates can depend on, without introducing a new external dependency.

## Decision drivers

- Avoid policy drift between runtime and macro crates.
- Preserve the public API of `rstest_bdd::execution` without adding new
  dependency cycles.
- Keep the solution internal to the workspace without introducing new external
  dependencies.

## Options considered

| Option                  | Pros                                   | Cons                                     |
| ----------------------- | -------------------------------------- | ---------------------------------------- |
| Keep duplicated enums   | No new crate                           | Drift risk; ongoing maintenance cost     |
| Move policy into macros | Single definition                      | Inverts layering; ties runtime to macros |
| New policy crate        | Single source of truth; clean layering | New crate to publish and maintain        |

Table: Options compared for policy type ownership.

## Decision outcome

The workspace now uses the internal crate `rstest-bdd-policy` to own
`RuntimeMode`, `TestAttributeHint`, and their associated helpers.

- `rstest-bdd-policy` is the single source of truth for policy types within
  the workspace.
- The runtime crate re-exports these types from `rstest_bdd::execution` to
  preserve the public API.
- The macro crate imports the types directly from `rstest-bdd-policy`, so it
  no longer defines local copies.

## Implementation status

- The crate exists at `crates/rstest-bdd-policy`.
- `rstest_bdd::execution::{RuntimeMode, TestAttributeHint}` remain stable
  public re-exports.
- `rstest-bdd-macros` imports the shared policy enums directly from
  `rstest-bdd-policy`.
- Regression tests in the runtime and proc-macro crates assert that both
  surfaces still use the shared types.

## Goals and non-goals

- Goals:
  - Eliminate duplicate policy enums.
  - Preserve existing public APIs.
- Non-goals:
  - Replace enum-based policies with trait-based policies in this change set.

## Migration plan

1. Phase 1: Introduce `rstest-bdd-policy` and migrate runtime/macro usage.
   Completed.
2. Phase 2: Document the accepted architecture and keep regression coverage in
   place so local enum copies are not reintroduced. Completed for the initial
   policy types covered by this ADR.

## Known risks and limitations

- Publishing order and versioning still affect downstream consumers because
  `rstest-bdd-policy` is versioned alongside the rest of the workspace.
- Future runtime-policy additions must continue to land in
  `rstest-bdd-policy` when they are shared between runtime and proc-macro
  layers, otherwise the original drift risk could return.

## Architectural rationale

- Centralizing policy types aligns with workspace layering constraints.

## Consequences

- The workspace gains one small, dependency-free crate.
- Macro and runtime crates share policy types without violating proc-macro
  dependency rules.
- The public API for `rstest_bdd::execution::RuntimeMode` and
  `rstest_bdd::execution::TestAttributeHint` remains stable.
