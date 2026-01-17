# Architectural decision record (ADR) 004: shared policy crate

## Status

Proposed.

## Date

2026-01-17.

## Context

The runtime crate (`rstest-bdd`) defines `RuntimeMode` and `TestAttributeHint`
in `rstest_bdd::execution`. The proc-macro crate (`rstest-bdd-macros`) cannot
depend on the runtime crate, so it maintains a parallel copy of those enums for
compile-time use. The duplication creates a manual synchronisation burden and
invites drift between the macro and runtime layers.

A single source of truth is required for the runtime policy types that both
crates can depend on, without introducing a new external dependency.

## Decision

Introduce a new internal crate, `rstest-bdd-policy`, that owns `RuntimeMode`,
`TestAttributeHint`, and their associated helpers.

- The runtime crate will re-export these types from
  `rstest_bdd::execution` to preserve the public API.
- The macro crate will import the types directly from
  `rstest-bdd-policy`, removing its local copies.

## Consequences

- The workspace gains one small, dependency-free crate.
- Macro and runtime crates share policy types without violating proc-macro
  dependency rules.
- The public API for `rstest_bdd::execution::RuntimeMode` and
  `rstest_bdd::execution::TestAttributeHint` remains stable.

## Alternatives considered

- Keep the duplicated enums and rely on manual synchronisation. Rejected due to
  drift risk and ongoing maintenance cost.
- Move policy into the macro crate and expose it from there. Rejected because
  it would invert layering and tie runtime logic to proc-macro concerns.
