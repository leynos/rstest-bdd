# Architectural decision record (ADR) 004: shared policy crate

## Status

Proposed

## Date

2026-01-17

## Context and problem statement

The runtime crate (`rstest-bdd`) defines `RuntimeMode` and `TestAttributeHint`
in `rstest_bdd::execution`. The proc-macro crate (`rstest-bdd-macros`) cannot
depend on the runtime crate, so it maintains a parallel copy of those enums for
compile-time use. The duplication creates a manual synchronization burden and
invites drift between the macro and runtime layers.

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

## Decision outcome / proposed direction

Introduce a new internal crate, `rstest-bdd-policy`, that owns `RuntimeMode`,
`TestAttributeHint`, and their associated helpers.

- Adopt `rstest-bdd-policy` as the single source of truth for policy types
  within the workspace, and re-export from `rstest_bdd::execution`.
- The runtime crate will re-export these types from
  `rstest_bdd::execution` to preserve the public API.
- The macro crate will import the types directly from
  `rstest-bdd-policy`, removing its local copies.

## Outstanding decisions

- Whether `rstest-bdd-policy` is published to crates.io or treated as a
  workspace-only crate.
- The naming and versioning policy for `rstest-bdd-policy` relative to the
  rest of the workspace releases.
- Whether future policy types should live in `rstest-bdd-policy` by default or
  remain owned by the runtime crate when they are runtime-only concerns.

## Goals and non-goals

- Goals:
  - Eliminate duplicate policy enums.
  - Preserve existing public APIs.
- Non-goals:
  - Replace enum-based policies with trait-based policies in this change set.

## Migration plan

1. Phase 1: Introduce `rstest-bdd-policy` and migrate runtime/macro usage.
1. Phase 2: Publish and document versioning policy.

## Known risks and limitations

- Publishing order and versioning may affect downstream consumers.

## Architectural rationale

- Centralizing policy types aligns with workspace layering constraints.

## Consequences

- The workspace gains one small, dependency-free crate.
- Macro and runtime crates share policy types without violating proc-macro
  dependency rules.
- The public API for `rstest_bdd::execution::RuntimeMode` and
  `rstest_bdd::execution::TestAttributeHint` remains stable.
