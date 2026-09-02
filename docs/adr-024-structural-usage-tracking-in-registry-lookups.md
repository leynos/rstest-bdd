# Architectural decision record (ADR) 024: Structural usage tracking in registry lookups

## Status

Accepted (2026-11-06), amended (2026-11-06): registry lookups return
`ResolvedStep`, which records usage when it is constructed. The projection
helpers remain current API rather than deprecated wrappers; see the amendment
under [Decision outcome](#decision-outcome).

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
(fuzzy text) return `Result<Option<ResolvedStep>, StepLookupError>`.
`ResolvedStep` dereferences to `Step`, exposes `as_step()` for an explicit
`&'static Step`, and records the step in the process-local used set when it is
constructed.

The seven projection helpers delegate to a metadata lookup and project one
field, so each inherits the usage-marking behaviour. `find_step_with_mode`
returns `Result<Option<&'static Step>, StepLookupError>` by projecting
`as_step()`.

The ownership boundary and permitted call sites are documented in the
developer's guide under "Registry lookup and usage-marking invariants".

### Amendment: fallibility, the scoped query, and the deprecation window

Three changes were made when the scoped step-library work (RFC 0001) merged
in. They keep Option C's structural guarantee while widening the result type
and adding one deliberately pure read.

1. **Fallibility.** A lookup now returns `Result<Option<_>, StepLookupError>`
   rather than `Option<_>`. Under scoped libraries, two definitions can be
   equally specific, and the scoped-vocabulary model treats that as an error.
   A bare `Option` would force the registry to pick a candidate, which is the
   behaviour RFC 0001 exists to prevent. The Option C guarantee is unchanged:
   a step is obtainable only through `ResolvedStep`, and `ResolvedStep` records
   usage on construction. Only the error channel is new.

2. **A pure scoped query.** `find_step_with_metadata_in_scope` takes an
   explicit `StepScope` and returns `Result<Option<&'static Step>,
   StepLookupError>` without constructing a `ResolvedStep`.
   This narrows Option B's rejection rather than reversing it. Option B was
   rejected because making *the public metadata pair* read-only would let a
   caller hold a resolved step that the diagnostic still reports as unused.
   The scoped query is not that pair: scenario execution owns it, it is the
   only caller, and execution marks the step immediately after resolution in
   `resolve_step_for_request`. The reason a `ResolvedStep` is not constructed
   there is ordering: execution must distinguish "resolved" from "not found"
   from "ambiguous" before it can decide that a step was used, and an
   `ExecutionError` must not record usage for a step that never ran. The public
   pair, used by `cargo bdd` and by external consumers, still records usage on
   construction.

3. **The deprecation window is dropped.** The projection helpers keep their
   names and are supported API. Deprecating them would have required sprinkling
   `#[expect(deprecated)]` across every in-repository caller and would have
   signalled removal in the next breaking release, which the scoped-libraries
   work does not ask for. They are still thin delegating wrappers, so there
   remains exactly one implementation of each projection and exactly one
   `mark_used` call site (`ResolvedStep::new`). The source compatibility that
   the deprecation window was meant to protect is delivered by keeping the
   helpers, not by postponing their removal.

## Known risks and limitations

- A metadata lookup is a query that mutates process-local bookkeeping. The
  mutation is deliberate and documented, but a caller that only wants to
  inspect the registry still records usage. Callers that need a genuinely
  read-only inspection should use `find_step_with_metadata_in_scope`.
- The used set is process-local and is not persisted across binaries.
- Two lookup families now exist: the usage-recording metadata pair and the pure
  scoped query. Which one to call is a per-call-site decision, so a new lookup
  path can still pick the pure form where recording was intended. The
  developer's guide states the rule; a code review should check it.
- `find_step_with_metadata_in_scope` takes a `StepScope`, while the metadata
  pair is global-only. Callers that need scoped lookup *and* usage recording
  compose the two explicitly.

## Architectural rationale

Usage tracking belongs to the value that resolution produces, not to each call
site. Returning a wrapper makes the invariant impossible to skip, and keeping
the projections as delegating wrappers preserves source compatibility while
leaving one implementation of each projection.

Fallibility composes with that guarantee instead of competing with it. The
outcome of a lookup is now a three-way sum — ambiguous, absent, or resolved —
and only the resolved arm can produce a `ResolvedStep`, so the "cannot obtain a
step without recording it" invariant is preserved by construction. The scoped
query is the one place where that invariant is deliberately relaxed, and it is
relaxed inside the execution boundary that immediately reasserts it.
