# Architectural decision record (ADR) 024: Structural usage tracking in registry lookups

## Status

Accepted (2026-11-06): registry lookups return `ResolvedStep`, which records
usage when it is constructed, and the projection helpers are deprecated.

## Date

2026-11-06.

## Context and problem statement

The step registry exposed eight lookup functions. Each returned a different
projection of the same resolved step: a synchronous handler, an asynchronous
handler, a handler paired with its execution mode, or a `&'static Step`. Usage
tracking for `unused_steps()` was a separate step that each caller had to
remember, so a new lookup path could resolve a step without recording it, and
the diagnostic silently under-reported.

Issue #753 asks for two things: collapse the lookups to a metadata-returning
pair, and make usage tracking structural by returning the step only through a
wrapper that records usage on construction.

## Decision drivers

- A resolved step must not be obtainable without recording its use.
- Callers that need one field should not each re-implement the projection.
- Existing callers should keep compiling through a deprecation window.
- The registry boundary should have one documented owner for usage marking.

## Options considered

### Option A: Keep the eight lookups and mark usage in each

Rejected. Every new lookup path is another place to forget the call, which is
the defect the issue reports.

### Option B: Make the metadata lookups read-only and mark usage at execution

Rejected. The issue requires usage tracking to be structural at resolution, and
`unused_steps()` is a diagnostic about resolution, not about execution. A
read-only lookup would also let a caller hold a resolved step that the
diagnostic still reports as unused.

### Option C: Return `ResolvedStep` from a metadata pair

Accepted. `ResolvedStep::new` records usage, and every other lookup is a
projection of that value.

## Decision outcome

`lookup_step_with_metadata` (exact pattern) and `find_step_with_metadata`
(fuzzy text) return `Option<ResolvedStep>`. `ResolvedStep` dereferences to
`Step`, exposes `as_step()` for an explicit `&'static Step`, and records the
step in the process-local used set when it is constructed.

The seven projection helpers remain as deprecated compatibility wrappers. Each
delegates to a metadata lookup and projects one field, so it inherits the
usage-marking behaviour. `find_step_with_mode` keeps its
`Option<&'static Step>` return type by projecting `as_step()`.

The ownership boundary and permitted call sites are documented in the
developer's guide under "Registry lookup usage-marking invariant". The
replacement for each deprecated helper is recorded in the v0.7.0 migration
guide.

## Known risks and limitations

- A metadata lookup is a query that mutates process-local bookkeeping. The
  mutation is deliberate and documented, but a caller that only wants to
  inspect the registry still records usage.
- The used set is process-local and is not persisted across binaries.
- The deprecated helpers must be removed in the next breaking release.

## Architectural rationale

Usage tracking belongs to the value that resolution produces, not to each call
site. Returning a wrapper makes the invariant impossible to skip, and keeping
the projections as delegating wrappers preserves source compatibility while
leaving one implementation of each projection.
